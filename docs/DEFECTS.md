# ClearDeck defect register

**One prioritised list.** Wave 1 produced five independent bodies of work (the `poker_core`
extraction, a differential evaluator harness, a PocketIC money-safety harness, a screenshot
harness and a reference corpus) plus five adversarial critiques. Several of them found the
same bug from different directions. This file reconciles all of it into one register, so wave 2
has a single queue to work from.

- Anything with fund impact also has a full write-up in **[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md)**.
  This file is the index and the priority order; that file is the evidence.

> ## WAVE 11, 2026-08-06 — THE REGISTER IS NOW TRUE, AND COUNTABLE BY A COMMAND
>
> **Before:** 150 rows spread over five tables, **71 distinct status strings** including eleven
> spellings of "fixed", 14 entries with a write-up and no row anywhere, two different defects both
> called `E-45`, and no way to answer "how much is open?" except by arguing. Nine of the twelve
> `SECURITY-FINDINGS.md` headers that did not say FIXED were **wrong in one direction or the
> other**, including [FINDING 15](SECURITY-FINDINGS.md#finding-15) (the fund lock, closed in wave 6)
> and [FINDING 17](SECURITY-FINDINGS.md#finding-17) (closed in wave 6/7).
>
> **After:** one table, [THE REGISTER](#the-register), five status words, the wave in its own
> column, **and every `FIXED` naming the gate that would catch it coming back.**
> `./scripts/register-stats.sh --check` counts it and fails if any of that stops being true.
>
> ```
> ./scripts/register-stats.sh          # at the close of the wave-11 register pass
>     DEFECTS.md — THE REGISTER      171 entries   52 OPEN  109 FIXED  6 FIXED-NO-GATE  4 BY-DESIGN
>     SECURITY-FINDINGS.md            39 findings    9 OPEN   29 FIXED                  1 BY-DESIGN
>     OPEN, both files:              1 critical · 19 high · 26 medium · 15 low  =  61 open items
> ```
>
> Those numbers are a reading, not a constant — three agents were writing to this file during the
> pass. **Run the command; do not quote this block.**
>
> ### Four things verifying the closed entries turned up, and three of them are the same thing
>
> **1. [H-45](#h-45) — six money-safety targets are named by no target, and they are the gates of
> the fourth and fifth cross-agent defects.** `stall_agreement` (M13 ONE BELIEF, [E-59](#e-59)),
> `solvency` ([E-70](#e-70)/[E-72](#e-72)), `deposit_subaccount_anchor`
> ([FINDING 28](SECURITY-FINDINGS.md#finding-28)) and `fund_reachability` (M9). **31 tests, run by
> nothing.** This is [H-17](#h-17) — *"the fund-theft gate was outside the gate"* — recurring four
> times in the four waves after the comment warning about it was written. All four were run by hand
> in this pass; every `FIXED` row whose gate says **`NOT RUN`** is one of them.
>
> **2. [H-48](#h-48) — one of those six is RED, and it is M9's own file.** 2 of 6 tests fail on
> the wasm `./scripts/dev.sh test` had just built. Both failures are at the predicate
> [E-59](#e-59) changed from the wall clock to ATTEMPTS. Probably a stale gate — `timers` is green
> and proves the money is reachable on a subnet — but **nobody knew either way, because nothing
> runs it.**
>
> **3. [E-41](#e-41) / [FINDING 39](SECURITY-FINDINGS.md#finding-39) — the canister owes 4,000,000
> e8s it does not hold, today.** Filed in wave 4 with a reproducer, never re-run, still red on this
> build, and **twice the size the entry records**. M1 CONSERVATION and M2 LEDGER REALITY, the two
> invariants this project calls never excusable. Same cause as the three above: no default target
> runs the configuration that finds it.
>
> > **WAVE 13: ROOT-CAUSED AND CLOSED. A HAND SETTLES TWICE.** Not a timeout defect and not a
> > two-seed artifact. `finish_hand` empties the pot and leaves `total_bet_this_hand` standing, so
> > between hands the table holds a complete payout basis over an empty pot; `use_time_bank` armed an
> > action clock on a finished hand, and the clock handed that hand back to `end_hand_single_winner`.
> > Every arithmetic post-condition passed on the way, because the plan conserves against its own
> > `collected`. Shrunk to **one limped heads-up hand and five ordinary player calls**; four guards,
> > four gates, each verified red in isolation. The sentence above about no default target running
> > the configuration was right and was also not the whole story: the fuzzer's ALPHABET could not
> > express the call ([H-26](#h-26)).
>
> **4. [FINDING 31](SECURITY-FINDINGS.md#finding-31) is genuinely open, driven again.** A deposit
> of exactly the advertised 20,000 e8s to the address the canister publishes still cannot be
> withdrawn.
>
> **What moved the other way:** [E-08](#e-08) (Candid drift) was closed by another owner in this
> same wave and is verified here at 0 structural differences; [E-11](#e-11), [E-43](#e-43),
> [E-46](#e-46), [H-14](#h-14), [H-18](#h-18), [H-19](#h-19), [H-24](#h-24), [H-36](#h-36) and
> [T-33](#t-33) were all marked or implied OPEN and are closed with named gates.
>
> **Read this before believing any `FIXED` row:** a status of `FIXED` in this file now means the
> gate column names something you can run. Where it says `none`, nothing would catch the defect
> coming back; where it says `NOT RUN`, something would, and no target invokes it.

> **WAVE 8, 2026-08-06.** A **fourth** independent auditor, run AFTER wave 8's fixes, still says
> **"NO — I would not tell a friend their money is safe here."** (verbatim) Full accounting with all four
> verdicts quoted, both answers for the lead, and every gate's result: **[WAVE-08.md](WAVE-08.md)**.
>
> **Six of auditor four's ten findings were already in this register before the wave started**,
> including the only one ever graded `fund-theft`
> ([FINDING 23](SECURITY-FINDINGS.md#finding-23), filed wave 7, untouched). The failure mode has
> changed: we are no longer mostly failing to *find* things, we are failing to *schedule* them.
> Read [WAVE-08.md](WAVE-08.md) §7 before planning wave 9.
>
> **THE FIFTH CROSS-AGENT DEFECT was hunted rather than stumbled on: [E-70](#e-70) /
> [FINDING 35](SECURITY-FINDINGS.md#finding-35) — CLOSED 2026-08-06, together with
> [E-72](#e-72) and [E-73](#e-73).** A live 2.00 ICP instance of it is on mainnet table_1 today
> and closing this does NOT return that money; it makes the canister able to see it and say it.
> Nothing was added that can edit a balance, and `no_setter_was_added_to_fix_the_books` is the
> gate that keeps it that way. Its signature is one level up from the
> previous four. Not "correct totals, wrong recipients", **a correct instrument pointed at a
> subset of the accounts.** The canister has an observation record for every deposit SUBACCOUNT
> and none for its MAIN account, so `admin_audit_deposit_custody` replies
> `(1 audited, 0 held, 0 unaudited)` on a canister holding 5 ICP and the currency guard accepts a
> flip that closes the money's only recovery door. Two more instances of the same shape are in
> the instruments: [E-72](#e-72) and [E-73](#e-73).
>
> **[FINDING 33](SECURITY-FINDINGS.md#finding-33) is CLOSED.** Three reviewers drove it and none
> left a gate; it is one word (`== Pull` under a comment saying `Pull` AND `Sweep`), it let a
> controller re-denominate a table holding 1.9999 ICP of a player's money, and **the flip could
> not be undone**. Fixed and gated by `tests/money_safety/tests/coherence_w8.rs`, which is named
> in `./scripts/dev.sh test`.
>
> **THE RENDERED NOTICE GATE RAN, for the first time in three waves: 24 shots, 14 verified,
> 10 red, and 5/5 protected notices on every one of the 24, behind both modals and under a real error toast.**
> It was dark because of [E-74](#e-74), not because of the replica. Four reds it could finally
> see: [E-71](#e-71), [E-75](#e-75), [E-76](#e-76) and [E-65](#e-65).

> **WAVE 9, 2026-08-06 — THE FRONTEND IS NOW BUILT FOR MAINNET THROUGH A NAMED PATH THAT
> VERIFIES ITS OWN OUTPUT.** `npm run build:mainnet` states the target in the command, refuses a
> contradictory `DFX_NETWORK`, checks every id against the ROLE the committed mapping gives it
> (T-01 in the direction nothing guarded), then reads the emitted JavaScript back and **deletes the
> dist if it does not verify**. 13 static checks, plus 23 rendered ones with mainnet made
> unreachable by two independent locks.
>
> **The first version of that verifier passed 12 of 12 while measuring nothing** — one wrong
> repetition count in a regex made `allPrincipals()` return an empty map, and the check
> "no local replica canister id in a mainnet bundle" printed a green tick over
> `checked 0 distinct canister id(s)` on a bundle containing eleven. The standing lesson of this
> repository, reproduced by the instrument written to enforce it, within the hour. There is a
> mutation self-test now (`build/verify-bundle.selftest.mjs`, 8 mutations, each caught by the
> named check) and a floor assertion so a scan that finds nothing fails instead of passing.
>
> Three new frontend defects, all found by rendering rather than by reading: [T-38](#t-38) (the
> "Deployed Canister Hashes" the app has been showing are three upgrades stale and nothing
> compared them to anything), [T-39](#t-39) (local sign-in navigates to
> `http://undefined.localhost:4943`), [T-40](#t-40) (**0 of 5 protected notices legible with the
> Verify Code dialog open, at both viewports** — HARD RULE 2, in the one dialog whose subject is
> whether this deployment can be trusted). All three are FIXED.
>
> **[E-70](#e-70) / [FINDING 35](SECURITY-FINDINGS.md#finding-35) now has a player-facing surface,
> and on today's deployment that surface reads as a WARNING**, because the honest answer is that
> the canister cannot say. See the E-70 entry.

> **WAVE 7, 2026-08-06.** A **third** independent auditor, run AFTER wave 7's fixes, still says
> **"No — I would not tell a friend their money is safe here."** Every blocker that stopped
> auditor two is closed. What blocks now is a different set, at a different boundary, and the full
> accounting with all three verdicts quoted is in **[WAVE-07.md](WAVE-07.md)**.
>
> **The fourth cross-agent defect was found, at the seam between the timers work and the
> custody-visibility work: [E-59](#e-59) / [FINDING 25](SECURITY-FINDINGS.md#finding-25). It is
> now FIXED (2026-08-06).** Same signature as the three before it — correct totals, wrong
> recipients, every invariant silent. After a stall, `abandon_stuck_hand`, `cash_out` and
> `leave_table` all voided a hand the on-chain clock plays out two rounds later, and
> `get_custody_status()` told the player who was about to lose it to do exactly that. There is one
> predicate now, it is about ATTEMPTS rather than the wall clock, and all six surfaces read it.
> Gated by **M13 ONE BELIEF** (`tests/money_safety/tests/stall_agreement.rs`), which compares the
> two paths on **138 forked states**: 118 rows paying different recipients → 0, and 20 `Err`
> replies that changed state → 0.
>
> **Two instrument defects also landed this wave and both are RED right now:** [E-60](#e-60)
> (`make hygiene` fails on a tracked 7.6 MB manifest) and [E-61](#e-61) (the no-rake gate reads a
> field that does not exist on the record it reads, so it cries wolf on every archived hand).
> **The rendered notice gate could not be run at all** — see [WAVE-07.md](WAVE-07.md) §4.

> **WAVE 7 INSTRUMENT PASS, 2026-08-06.** [E-60](#e-60) and [E-61](#e-61) are **CLOSED**, and so is
> the fund lock the third auditor could not get his money out of ([E-62](#e-62) /
> [FINDING 27](SECURITY-FINDINGS.md#finding-27)): the two floors are one number per currency, the
> relation is a **compile-time** assertion, and a player's whole remaining balance can always
> leave whatever its size.
>
> **The more useful finding is why a real regression was walked past for a whole wave.** The pixel
> gate DID catch [E-63](#e-63) — 8.7% of a money figure painted over by the felt at 390x844 — and
> nothing anybody could run was red about it, because the only instrument that can see a rendered
> failure needs a live replica and the replica has been down for two waves. The last recorded
> verdict is now itself a gate: `./scripts/dev.sh shots-verdict`, which `make hygiene` runs, reads
> the tracked `artifacts/screens/latest/verdicts.json` and fails on any red that is not written
> down in `acknowledged-reds.json` under a defect in this file — **and on any acknowledgement that
> is no longer red**, so the ledger cannot rot into a list of permanent excuses.
>
> [E-63](#e-63) and [E-64](#e-64) are fixed but **not re-measured on rendered pixels**; both are
> acknowledged as still-red until a sweep says otherwise. [E-65](#e-65) (the lobby quotes blinds
> the canister does not have) is newly filed and OPEN.

> **WAVE 6, 2026-08-05.** A second independent auditor, run AFTER wave 6's fixes, still says
> **"NO — I would not tell a friend their money is safe here."** The full accounting, both
> auditors' verdicts quoted, every gate's verdict and the shortest path to a different answer are
> in **[WAVE-06.md](WAVE-06.md)**. The four open fund-safety blockers were
> [FINDING 07](SECURITY-FINDINGS.md#finding-07) (critical: one controller call destroys a table's
> chips), [FINDING 18](SECURITY-FINDINGS.md#finding-18), [FINDING 19](SECURITY-FINDINGS.md#finding-19)
> and [FINDING 17](SECURITY-FINDINGS.md#finding-17).
>
> **[FINDING 18](SECURITY-FINDINGS.md#finding-18) is CLOSED (2026-08-05, [E-57](#e-57)).** A player
> can no longer walk away from a table with money in the pot and be told they have nothing:
> `cash_out` and `leave_table` settle a hand no message can move before they vacate the seat, a
> live hand cannot outlive its last player, and `get_custody_status`, the table view and
> `withdraw`'s refusals all state the stake and name `abandon_stuck_hand`. Gated by **M10 CUSTODY
> VISIBILITY**, which reads the canister as the PLAYER rather than as a controller.
- **Do not fix a defect without a test that fails before the fix.** (Wave 1 deliberately froze
  engine behaviour so ground truth could be established first; that freeze ended in wave 2 and
  most of this file is now closed work. What is left is [THE REGISTER](#the-register)'s `OPEN`
  rows.)
- Reproduce anything with `make test` (fast) or the exact command on each entry.
- Count anything with `./scripts/register-stats.sh`.

## How to read this register

**Everything is in ONE table, [THE REGISTER](#the-register), and the status column has five
words in it. That is new, and it is the point.**

For eight waves this file counted its own work in eleven different spellings of "fixed"
(`FIXED IN THIS PASS`, `FIXED-IN-WAVE-2`, `FIXED-IN-COHERENCE-PASS`, `fixed-in-wave-1`,
`MOSTLY-FIXED-IN-WAVE-2`, `PARTLY-FIXED-IN-WAVE-2`, `CLOSED`, `answered`, `partly fixed`,
`FIXED for X, OPEN for Y`, `open — other owner`) spread over **71 distinct status strings** in
five separate tables, fourteen entries had no row in any table at all, and two different defects
were both called E-45. Nothing could count it. The consequence is on the record: six of one
auditor's ten findings were **already filed here** before that wave started, and the coherence
pass's verdict was *"we have stopped failing to FIND things and started failing to SCHEDULE
them."* A register you cannot count is a scheduling hazard, not a document.

### The status vocabulary — these five words and no others

| status | means |
|---|---|
| `OPEN` | the defect is present today. |
| `FIXED` | the defect is gone **and** the gate column names the thing that would catch it coming back. |
| `FIXED-NO-GATE` | the defect is gone and **nothing would catch it coming back.** A fix with no gate is a memory, so it gets its own word rather than hiding inside `FIXED`. |
| `WONTFIX` | a real defect this project has decided not to fix. The entry says why. |
| `BY-DESIGN` | investigated and not a defect, or the behaviour is intended. The entry says why. |

Two rules make the vocabulary hold:

1. **The wave lives in its own column**, never inside the status. "Fixed in wave 2" is two facts.
2. **There is no `PARTIAL`.** An entry with one half landed and one half live is `OPEN`, and the
   entry says which half landed. A partial status is a place for work to go and not come back:
   `FIXED for the archive wiring, OPEN for the lobby wiring` sat in this file for four waves and
   was counted as neither.

### The gate column is the load-bearing one

`FIXED` without a named gate is a claim about the past. The gate column names the command and the
test, so a stranger can re-run it:

* `` `dev.sh test` → `regressions::reg01` `` — `./scripts/dev.sh test`, that target, that test.
* `sweep: …` — the screenshot harness's recorded verdict. `./scripts/dev.sh shots-verdict` (which
  `make hygiene` runs) gates on the last recorded sweep with no replica; `./scripts/dev.sh shots`
  re-measures it with one.
* `CI …` — a job in `.github/workflows/ci.yml`.
* `none` — the `FIXED-NO-GATE` marker.
* **`NOT RUN`** — the gate was written, it exists, and **no target invokes it.** Six money-safety
  targets are in this state right now and they carry the named gates of the fourth and fifth
  cross-agent defects: [H-45](#h-45). One of the six is also RED: [H-48](#h-48).

### Counting is a command, not an argument

```
./scripts/register-stats.sh            # the counts, and the open list worst-first
./scripts/register-stats.sh --check    # the same, plus the consistency gate (exit 1 on a problem)
./scripts/register-stats.sh --open     # just the open ids, for scripting
```

`--check` fails on a status outside the vocabulary, on a `FIXED` row that names no gate, on an
`OPEN` row that claims one, on an anchor that does not resolve, on a duplicate id, and — the one
that would have caught the fourteen uncounted entries — on **any entry with a write-up and no row
in the table**. It earned its keep inside the pass that wrote it: three entries added by another
owner while it was running were caught the same hour, with no row and no anchor.

> **This script is not wired into any gate, and that is [H-45](#h-45) waiting to happen to it.**
> `./scripts/register-stats.sh --check` needs no replica, no wasm and about a second, so it belongs
> as a step in `cmd_hygiene` — one line in `scripts/dev.sh`, which the register pass does not own.
> Until that lands, a wave that adds an entry without a row will not be told.

## ID scheme

| prefix | domain |
|---|---|
| `E-` | engine / canister. Real money. |
| `T-` | build, deploy or configuration of the app itself. |
| `H-` | the harnesses. A harness defect means a result you cannot trust. |
| `D-` | documentation and evidence: a claim the artifact does not support. |
| `L-` | the lobby screen and what it renders (wave 5). |

Ids are permanent and are never reused. `E-45` was assigned twice by two agents in different
waves; the second one (the archived-block defect) is now **[E-77](#e-77)**.

Anything with fund impact also has a full write-up in
**[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md)**, whose findings table uses the same five-word
vocabulary and the same gate column.

---

> ## WAVE 12, 2026-08-07 — THE DISCLOSURE WAS FALSE IN THE OPERATOR'S FAVOUR
>
> Full accounting: **[WAVE-10.md](WAVE-10.md)** (the tenth wave *document*; the register's `wave`
> column runs ahead of it and these rows say `12`).
>
> **[D-14](#d-14), graded `fund-theft`.** Wave 12's custody disclosure — the largest single
> improvement in this tree — told a depositor, on the deposit screen and in the README's decision
> table, that the operator could destroy their balance but could not take it. **A controller
> replaces the code**, so a 500-byte module installed with the same verb as the wipe moved
> **39.99990000 ICP** into a wallet the operator owns. Every presence check in the hygiene gate
> was GREEN while the paragraph lied, so the retraction has an INVERTED gate of its own.
>
> **[E-86](#e-86) is the EIGHTH cross-agent defect** and the purest instrument-side instance of
> the signature yet: the deposit gate compared four correct screen figures against three wrong
> chain quantities because a copy edit added a fourth number, and reported it as a product defect
> with real numbers attached.
>
> **Five gates that nothing ran** — [H-50](#h-50) (`tools/archive`, 39 cases),
> [H-51](#h-51) (`table-in-frame.mjs`), [H-52](#h-52) (the guardian's `.did` vs its wasm),
> [H-53](#h-53) (`tests/no_peeking`, 36 tests) and [H-47](#h-47) (two divergent self-test lists) —
> are named by a target now. [H-53](#h-53) is the one worth reading: its manifest names all five
> targets explicitly *so that nothing would be auto-discovered*, and then no human ever typed one.

> ## WAVE 14, 2026-08-09 — THE COHERENCE PASS: FOUR DOORS, AND THE ONE NOBODY OWNED
>
> Five builder/critic pairs worked this tree at the same time. Each pair's own work
> holds: the dead-band refund, the deposit trust root, the single definition of
> held and owed, the poll split, and the gates that run all of it were each measured
> red before and green after. **Every defect this pass found lives between two of
> them.**
>
> **[T-47](#t-47) / [FINDING 45](SECURITY-FINDINGS.md#finding-45), graded
> `fund-theft`.** The deposit stream closed three money doors against a table id
> that arrives over an uncertified query, wrote a gate called
> `every_money_door_in_the_modal_refuses_an_unpinned_table`, and shipped a summary
> saying *"all three money doors refuse together, because closing one and leaving
> the others is exactly how FINDING 40 shipped half closed."* There were four.
> `loadBtcDepositAddress()` asks the wire-supplied canister for a **Bitcoin**
> address and the modal renders it under *"Your Bitcoin Deposit Address"* with a
> Copy button and no trust check anywhere in that subtree. It is worse than the
> defect it sat beside: the address is not derived from the canister id, it is
> **fetched from the canister** — [FINDING 40](SECURITY-FINDINGS.md#finding-40)
> unfixed, on a chain where the transfer cannot be reversed. The same substituted
> reply picks the branch, because `currency` comes out of it too.
>
> **[H-63](#h-63): two gates on one directory, one green and one red, and CI ran
> the red one.** Each `src/declarations/<n>/` holds three copies of an interface.
> The new generator writes two of them; the new gate reads the same two; the older
> gate that CI runs reads the third. When the lobby gained two methods,
> `check-declarations-js.sh` printed *"all 5 binding set(s) regenerate to exactly
> what is committed"* while `check-candid.sh --declarations` exited 1 on the same
> directory in the same tree. The stream that saw the red attributed it correctly
> to another stream; the stream that owned the change was reading the gate that
> could not see it. **Two correct local judgements, one red CI.**
>
> **[H-57](#h-57): FINDING 31's dead band moved rather than closed.** The refund
> genuinely reaches every amount above one ledger fee at a deposit subaccount, and
> that is measured at the player's own wallet. But `check_drain`'s tolerance was
> rewritten to convict every account above one fee while `drain()`'s own withdraw
> loop kept knocking at the old `bal > 20_000`, and the canister pays any whole
> balance above one fee. Every escrow row in `(10_000, 20_000]` was therefore
> reported unreachable **by an instrument that never asked for it** — a false red
> on M9, the top-severity invariant here, in a repository that keeps an
> acknowledged-reds file because reds get skimmed. The drain transcript for the
> reproducer is empty.
>
> **[T-48](#t-48): the poll split rebuilt FINDING 15 on the client.** Splitting the
> 500 ms poll into queries plus a 2 s clock nudge was right and cut a measured
> 1.1380 T/day per tab to 0.0572. The nudger's `seated` term for an unreadable view
> read `this.lastCallAt !== null`, and `lastCallAt` is only set when a call is
> made — so a tab that never gets a decodable `TableView` never calls, is never
> seated, never reaches the backstop, and emits **zero `check_timeouts` in a day.**
> The client it replaced called `check_timeouts` before `get_table_view`, so it kept
> firing exactly in that state. Three lines above the defect, the comment reads:
> *"'I could not tell' must not become 'so I stopped calling'."* **The comment
> stated the requirement and the code did the opposite**, and the gate beside it
> asserted the predicate was false and called that "the backstop covers it" without
> measuring whether the backstop fired.
>
> ### THE STANDING LESSON OF THIS WAVE
>
> The repository already knows that an instrument measuring nothing passes. Wave 14
> produced the sharper form: **an instrument can be weakened by a correct fix to
> something else, and it reports the same word.**
>
> * [H-59](#h-59): replacing sixteen `cargo test --test X` lines with a `run_ms`
>   helper — a clean refactor — took `check-suite-wiring.sh`'s fourth assertion from
>   23 subjects to **7**. It printed `ok` both times. Only the count in the message
>   changed, and nothing reads the count. With zero subjects it passed as well. This
>   is the gate built to catch H-45, blinded by the fix for H-45.
> * [H-56](#h-56): the FINDING 42 door gate compared the byte OFFSET of the guard
>   against the byte offset of the approval. Deleting the single token `return;`
>   left all seven tests green **and** the browser reproducer exiting 0 printing
>   `✓ REFUSED`, while the approval went out to the substituted canister. **The
>   first repair was also too weak and was caught only by re-running the same
>   mutation** — "a `return` appears between the check and the money call" is
>   satisfied by the `return` belonging to the next statement.
> * [H-61](#h-61): `test-burn-table.mjs` verifies that `burn-table.json` agrees with
>   itself and never reads the 13 raw runs beside it. Divide every per-tab price by
>   four, recompute consistently, and the gate is green while the monitor's alarm
>   floor drops by 70%.
>
> So the wave's rule, in the same shape as the one about anchors: **a gate's subject
> list is part of the gate.** Ask what it is looking at, not only what it asserts,
> and prove it by breaking the thing it claims to watch — not by reading it.
>
> ### What this pass changed, and what it did not
>
> Closed here: [T-47](#t-47), [T-48](#t-48), [H-56](#h-56), [H-57](#h-57),
> [H-59](#h-59), [H-63](#h-63). Filed and left open, with reasons in their rows:
> [H-58](#h-58) (nothing ties the shipped page to the clock policy, so every
> published per-tab price rests on an unenforced link), [H-60](#h-60),
> [H-61](#h-61), [H-62](#h-62), [H-64](#h-64) (the drain's stranded leg still reads
> the canister's own books while the ledger scan sits unused in the same struct),
> [H-65](#h-65) and [E-101](#e-101).
>
> Rendered-pixel evidence for this pass: a full 24-shot sweep after every edit here
> — **19 verified, 5 red, all 5 the same reds the streams recorded**, so nothing in
> this pass added one. `5/5 protected notices on screen` at 1440x900 and 390x844,
> `notices survive an error toast`, and the four notices plus the no-rake sentence
> read legibly off the mobile PNG. The deposit shot's two reds are [E-101](#e-101).
>
> Corrections to claims this wave made and could not support, each written into the
> entry a reader would hit: [FINDING 42](SECURITY-FINDINGS.md#finding-42)'s "all
> three doors" and its "every agent"; [FINDING 38](SECURITY-FINDINGS.md#finding-38)'s
> outside anchor, which anchors the held side only; [FINDING 31](SECURITY-FINDINGS.md#finding-31)'s
> tolerance, which turned on whether the drain tried rather than on recoverability;
> [E-100](#e-100)'s "56x error", which is a fixture result — on the live fleet the
> reported days are byte-identical either way; [E-92](#e-92)'s "typical" per-tab
> price, measured on a table where the mechanism being priced fired zero times; and
> [T-01](#t-01), which is `FIXED` behind a gate that exits 1 on every run
> ([T-46](#t-46)) — something `register-stats.sh --check` is structurally unable to
> notice, because it asks whether a `FIXED` row names a gate and never whether the
> gate passes.
>
> Figures in earlier wave-14 text that this pass could not reproduce are left
> corrected rather than deleted: the table wasm on this tree is
> `ff98f6640c93fb4683732ff31c6b4011c2efdc1d27601ec142cf9a06e86163a2`, not the
> `6902bb50…` one stream recorded before four others edited `lib.rs`; and the "nine
> stale `NOT RUN BY ANY TARGET` cells" were six literal cells across eight ids.

<a id="the-register"></a>
## THE REGISTER

**Sorted worst-first: everything `OPEN` before everything closed, by severity inside that.**
The sub-tables further down this file are the per-wave queues as they were written at the time;
they are historical and their status columns are NOT maintained. This table is the only one that
is true.

| id | sev | status | wave | gate — what would catch it coming back | where | one line |
|---|---|---|---|---|---|---|
| [E-102](#e-102) | high | OPEN | — | — (**the gate exists and is RED**: `./scripts/check-fleet-coherence.sh --network ic`, committed at `eb819a8`. The row stays gateless because the DEFECT is live on mainnet: a gate that reports a split fleet does not un-split it. It flips to FIXED when the fleet is redeployed from one build and the same command exits 0) | the mainnet deployment itself; `scripts/check-deployed.sh` and `scripts/check-deployed-config.sh`, which are both per-canister | **two different engines are running on mainnet right now, and no gate in this repository could ever have said so.** `table_1`, `table_2`, `table_3` and `btc_table_1` are four instances of ONE package, `table_canister`. They differ only in `init_args`, which are install-time arguments and not part of the module, so all four **must** report the same module hash. Certified state says `table_1` is `4511ab187cff8b90…` and the other three are `9c0ed3a138a3753d…`. Whoever sits at the older table plays with every money defect closed since that build. It happened when a deploy went out from a tree that was being edited ([E-79](#e-79)) and it survived every wave since, because `check-deployed.sh` compares each canister against an expectation and `check-deployed-config.sh` compares each canister against `icp.yaml` — **both are per-canister, so a fleet that is individually plausible and collectively incoherent passes both.** It also breaks the only trust story this project has: `README.md` tells a stranger to build the Docker image and compare hashes, that image builds `table_canister` once, so a verifier following our own instructions matches one canister of four and correctly concludes the rest are not the code we published |
| [E-103](#e-103) | high | FIXED | 14 | `./scripts/check-fleet-coherence.sh --network ic` reads CERTIFIED state via `dfx canister info --identity anonymous`, and its `module_hash()` carries the measurement that retired the dashboard. Verified by construction: the dashboard path is gone, so it cannot be silently reintroduced without editing a function whose comment is the incident | `README.md` "Verify the Code"; any verifier who reads `https://ic-api.internetcomputer.org` | **the source we point a stranger at for "what the IC reports" served a hash that was silently stale, and it changed the answer.** Building [E-102](#e-102)'s gate on the public dashboard API looked right — it needs no key, so anyone on earth can run it. Within one hour the same endpoint returned two different module hashes for `table_3` (`511c9d0e6d508d62…` then `9c0ed3a138a3753d…`) with **nothing deployed in between**: every deploy in that window was `-e local`, confirmed by scanning the five builders' transcripts. The dashboard is an INDEX over the IC, it lags, and it is not certified. The first reading of [E-102](#e-102) was consequently wrong in public — reported as three engines when certified state says two — and the error direction is the dangerous one in both senses: a verifier can read fraud where there is none, or all-clear over a split fleet. **A trust instrument may not rest on an uncertified cache.** This is [FINDING 42](SECURITY-FINDINGS.md#finding-42)'s lesson (*the wire is not the trust root*) arriving at the verification path instead of the deposit path, one wave later, found by an instrument written for a different defect |
| [D-14](#d-14) | fund-theft | FIXED | 12 | `dev.sh hygiene` -> `the retracted custody claim has not come back (FINDING 23c)` (an INVERTED check: the two retracted sentences must be absent from `README.md` and the frontend) + `dev.sh custody` -> `finding23c_the_operator_can_pay_the_ledger_balance_to_themselves`, which executes the theft | `DepositModal.svelte` `.custody-notice`; `README.md` "What that means for a deposit" | **the custody disclosure understated a fund-THEFT capability as destruction, on the screen a player deposits from.** It promised that the money would be "unreachable by anybody, including the operator", and the README's decision table answered "Can the operator take the ICP out of the canister to their own wallet?" with **No**. Both false. A controller is not bound to the ClearDeck wasm: a 500-byte module installed with the SAME verb as the wipe moved **39.99990000 ICP** into a wallet the operator owns. Every presence check in [D-13](#d-13)'s gate was GREEN while the paragraph lied, which is why the retraction needed an inverted gate of its own |
| [H-65](#h-65) | medium | OPEN | — | — | `tools/shots/lib/chain-agreement.mjs` `~1518`, the fiat leg | **the deposit shot has an intermittent FALSE RED, produced by the harness's own read ordering.** The fiat check re-reads the SERVED QUOTE after the settle loop — its comment says *"a quote can land between the two"* — and then compares it against `dom.usdValues`, which was read BEFORE the loop and is never re-read. When the third-party quote lands in that window the harness holds a quote and a stale-empty DOM, and reports `DEPOSIT CHAIN DISAGREEMENT: a live quote (2.19 USD/ICP) was served but the modal shows no fiat figure`. **Observed once by the wave-14 coherence pass** on `deposit/desktop` and not reproduced in the immediately following full sweep on the same tree, same replica, same fixture. It lands on the deposit screen, which is already carrying an acknowledged red ([E-101](#e-101)) — so the failure mode is that an intermittent red arrives on a shot people have learned to expect a red on, and the `covers` rule then fails the whole gate for a reason that is not real. The fix is to re-read the DOM in the same breath as the quote |
| [H-64](#h-64) | medium | OPEN | — | — | `tests/money_safety/src/invariants/reachability.rs` `DrainReport::stranded_breakdown()`; `Snapshot` in `tests/money_safety/src/world.rs` | **the drain's stranded leg is measured against the CANISTER, not the ledger — this repository's standing lesson, re-created inside the fix for that lesson.** `stranded_breakdown()`'s deposit term reads `DrainReport::deposit_custody_after`, which is `Snapshot::canister_deposit_by_principal`: the reply from `admin_deposit_custody()`, i.e. **what the canister says it observed**. The escrow term is `admin_get_all_balances()`, the canister's own escrow book. The same `Snapshot` already carries `ledger_deposit_by_principal`, built per principal from `icrc1_balance_of`, and its own doc comment says *"The whole value of this field is that it can DISAGREE with the ledger scan"* — and `stranded_breakdown()` does not consult it. So wave 14's claim that *"recoverability is measured at the player's wallet on the ledger, never at an escrow row"* is true of the `deposit_floor` tests and false of `check_drain`'s stranded leg, which is the leg that decides whether `fuzz-default` is red. **Not blind today:** `orphaned_e8s()` is ledger-anchored (but AGGREGATE, and reduced by `Exemptions::unobserved_subaccount`) and `check_deposit_attribution` backstops it. The fix is to iterate the union of both maps and take the MAX per principal, which cannot under-report |
| [T-47](#t-47) | fund-theft | FIXED | 14 | `cd tests/money_safety && cargo test --test deposit_trust_root -- every_money_door_in_the_modal_refuses_an_unpinned_table` — verified RED by removing the two guards on the tree that has them: *"loadBtcDepositAddress() asks an unpinned canister for a Bitcoin address (guard at 6033, fetch at 1110)"* | `DepositModal.svelte` `loadBtcDepositAddress()` (the fetch) and the `{#if isBTC && depositMethod === 'btc'}` subtree (the render) | **wave 14 closed three money doors in the deposit modal and left a fourth open, and the fourth is the worst of them.** [FINDING 42](SECURITY-FINDINGS.md#finding-42) pinned the table id so the ICP address, the OISY transfer and the ICRC-2 approval all refuse an unpinned canister. `loadBtcDepositAddress()` was not among them: it calls `get_btc_deposit_address()` on the wire-supplied id and the template renders whatever string comes back under **"Your Bitcoin Deposit Address"** with a Copy Address button, with no trust check anywhere in that subtree. It is worse than FINDING 42 because the address is not DERIVED from the canister id, it is **FETCHED from the canister** — [FINDING 40](SECURITY-FINDINGS.md#finding-40) unfixed, for a chain whose transfers cannot be reversed. The attacker picks the branch too: `currency` comes off the same uncertified `get_tables()` reply. The gate that missed it is named `every_money_door_in_the_modal_refuses_an_unpinned_table`. Full write-up: [FINDING 45](SECURITY-FINDINGS.md#finding-45) |
| [T-48](#t-48) | high | FIXED | 14 | `node tools/shots/test-clock-nudge.mjs` → `a tab that never gets a readable view still nudges the clock (T-48)` — verified RED by restoring `this.lastCallAt !== null`: `FAIL a tab that never gets a readable view still nudges the clock` | `src/cleardeck_frontend/src/lib/clockNudge.js` `decide()`, the `seated` term | **the poll split made a tab that cannot read the table stop asking the canister to advance its clock — which is [FINDING 15](SECURITY-FINDINGS.md#finding-15)'s fund lock, rebuilt on the client side.** `seated` for a null view read `this.lastCallAt !== null`, which sounds like *"we have seen a table before, so keep a pulse"* and is not: `lastCallAt` is only set when a call is MADE, so a tab that never gets a decodable `TableView` has never called, is never seated, never reaches the 60 s backstop, and emits **zero `check_timeouts` for as long as it stays open** — measured, 0 calls in a synthetic day. The client this replaced called `check_timeouts` as the FIRST statement of the 500 ms poll, *before* `get_table_view`, so it kept firing at 2/s exactly when the view could not be read. `check_timeouts` records `note_stall_opportunity`, and three of those across the grace period is what makes `abandon_stuck_hand` reachable when the on-chain timer is dead. The module's own header says a tab that stops asking rebuilds that lock, and the comment three lines above the defect said *"'I could not tell' must not become 'so I stopped calling'"* — **the comment stated the requirement and the code did the opposite**. Now 1,440 calls/day worst case, 0.0099 T/day, inside the gate's 0.02 T budget |
| [H-56](#h-56) | high | FIXED | 14 | `cd tests/money_safety && cargo test --test deposit_trust_root -- every_money_door_in_the_modal_refuses_an_unpinned_table` — the assertion is now on the guard's OWN brace-matched block; verified RED by deleting the single token `return;` | `tests/money_safety/tests/deposit_trust_root.rs` `every_money_door_in_the_modal_refuses_an_unpinned_table` | **the gate that certified FINDING 42 closed measured TEXT POSITION, not control flow.** It found the byte offset of `!tableIsTrusted` inside `handleDeposit` and asserted it was lower than the offsets of `icrc2_approve` and `wallet.transfer` — which is equally true of `if (!tableIsTrusted) { error = msg; }` with no `return`, a guard that sets a message and signs anyway. Deleting that one token left all seven tests green AND left `node tools/shots/repro-finding42.mjs` exiting 0 printing *"REFUSED"*, because the reproducer scrapes only the Claim button's `disabled` and never observes the Deposit button or the approval. Nothing asserted the `disabled` bindings at all. **The first repair was itself too weak and was caught by re-running the same mutation:** "a `return` appears between the check and the money call" stayed GREEN, because the next `return` belongs to the `if (!depositAmount ...)` validation immediately below. The assertion had to be scoped to the guard's own block |
| [H-57](#h-57) | high | FIXED | 14 | `cd tests/money_safety && cargo test --test fund_reachability -- the_drain_tries_every_balance_the_canister_would_pay` — verified RED at `> 20_000`: `the drain left 15000 e8s convicted as unreachable … left: 15000 right: 0`, with an EMPTY drain transcript, and green after with the player's own ledger wallet up exactly 5,000 | `tests/money_safety/src/invariants/reachability.rs`, the escrow leg of `drain()` | **M9, the top-severity invariant in this harness, was made able to convict money the canister pays on the very next call.** Wave 14 replaced `check_drain`'s aggregate tolerance with a PER-ACCOUNT rule at one ledger fee and left the drain's own withdraw loop knocking at `bal > 20_000`, the old `min_withdrawal + fee`. The canister waives its policy minimum for a whole-balance sweep (`sweeping_whole_balance = amount == balance_now && amount > transfer_fee`), so **every escrow row in `(10_000, 20_000]` was reported `FundsUnreachable` by an instrument that never asked for it.** This is [FINDING 31](SECURITY-FINDINGS.md#finding-31)'s dead band MOVED rather than closed: out of the deposit subaccount, where `refund_external_deposit()` now reaches it, and into the measuring instrument, where it produces a FALSE RED on `dev.sh test` and `fuzz-default`. In a repository that keeps an acknowledged-reds file precisely because reds get skimmed, an invariant that cries wolf is the failure mode being defended against. The threshold is now the canister's own, not a constant of the harness |
| [H-63](#h-63) | high | FIXED | 14 | `./scripts/check-candid.sh --declarations` (CI job *Candid interface drift*, `.github/workflows/ci.yml`) — it was exit 1 on the wave-14 tree with two NEW drift entries and is exit 0 now | `tools/gen-declarations`, `scripts/check-declarations-js.sh` and `scripts/check-candid.sh --declarations`, all reading `src/declarations/<n>/` | **two gates on the same directory disagreed, one green and one red, and the red one is the one CI runs.** Wave 14 built a generator for the frontend bindings and a gate around it; both write and read `<n>.did.js` and `<n>.did.d.ts` and **neither touches `<n>.did`**, the Candid TEXT in the same directory, which is what the older `check-candid.sh --declarations` compares. So when the lobby gained `refresh_table_config` and `refresh_all_table_configs`, `check-declarations-js.sh` reported *"src/declarations/lobby matches src/lobby_canister/lobby_canister.did"* while `check-candid.sh --declarations` reported *"NEW declaration drift, not in the baseline"* and exited 1. It was left that way because the one stream that saw the red correctly attributed it to another stream's lobby change and the stream that owned the lobby change was reading the gate that could not see it. **Nothing regenerates that file; it is hand-edited, and that is now written down rather than assumed** |
| [H-58](#h-58) | high | OPEN | — | — | `tools/shots/test-poll-updates.mjs`, `tools/shots/test-clock-nudge.mjs`, `+page.svelte` `advanceTableClock()` | **nothing ties the shipped page to the clock policy, so every measured cycles claim rests on a link no gate enforces.** `test-poll-updates.mjs` checks only the TIMER PERIOD (2000 ≥ the 2000 ms floor) and a shape budget (51,840 < 60,000); `test-clock-nudge.mjs` exercises `clockNudge.js` in isolation and never reads `+page.svelte`; `test-burn-table.mjs` reads only JSON. Measured: a `+page.svelte` that never consults `clockPolicy` and calls `check_timeouts` unconditionally on its 2 s timer leaves **all three gates green**, at 43,200 calls/day/tab ≈ 0.298 T/day — 5.2x the published "typical" price and 1.54x the published policy CEILING, which is the figure `fallback_burn_per_day` is built from. [T-48](#t-48) is the same seam from the other side: the policy was wrong and only a test of the policy could see it. The honest fix is a gate that resolves `advanceTableClock`'s body and requires the decision to gate the call, which is the third instrument this pass would have had to write |
| [E-101](#e-101) | medium | OPEN | — | — | `artifacts/screens/acknowledged-reds.json`, `SolvencyNotice.svelte`, `tools/shots/lib/chain-agreement.mjs` | **one register id is being used to excuse two OPPOSITE symptoms on the same screen.** [E-96](#e-96) says money the table cannot attribute *"renders NOTHING on the deposit screen"*; `acknowledged-reds.json` cites **E-96** to excuse `deposit/desktop` and `deposit/mobile` going red with *"TOKEN CENSUS FAILED: 8 of 43 numeric tokens on screen are asserted by nothing"* — money figures that DO render, one of them (`287.9296`) inside the `p.advice` sentence wave 14 rewrote. Both are real; they are not the same defect, and a reader who follows the acknowledgement to E-96 is told the screen shows nothing. The census red needs its own id, its own site in `chain-agreement.mjs`, and the acknowledgement repointed. Until then `./scripts/dev.sh shots` exits 1 with the deposit screen UNVERIFIED at both viewports, where at `a65868e` it was VERIFIED with *"0 unaccounted"* |
| [H-59](#h-59) | medium | FIXED | 14 | `./scripts/check-suite-wiring.sh` — check 4 now recognises the `run_ms` helper as well as a literal `--test`, and REFUSES to pass on an empty subject list. It reported `7 target(s) named in dev.sh` before and reports `25` now | `scripts/check-suite-wiring.sh`, check 4 | **the fix for [H-45](#h-45) silently blinded 70% of the gate built to catch H-45.** Check 4 (*"every target `scripts/dev.sh` names with `--test` has a row"*) discovers its subjects with `grep -oE -- '--test [a-z_0-9]+' scripts/dev.sh`. Wave 14 replaced sixteen literal `cargo test --test X` lines in `cmd_test` with `run_ms X`, so those strings stopped existing: the check went from 23 subjects to 7 and printed `ok` both times — **only the number in the message changed, and nothing reads the number.** Compounding it, with ZERO subjects it still passed, because `printf '%s\n' "" \| wc -l` is 1. It is also the only one of the gate's five assertions with no `--selftest` case; the selftest plants nine failures covering checks 1, 2, 3 and 5. An instrument that quietly measures less and passes, introduced into the gate written to stop exactly that |
| [H-60](#h-60) | medium | OPEN | — | — | `scripts/check-deployed-config.sh`, the lobby-vs-contract loop | **the new lobby leg prints `✓ matches` when it compares ZERO fields — [H-55](#h-55)'s exact shape, in the same file that was just fixed for H-55.** The loop does `IFS=',' read -ra pairs <<< "$tfields"`; if the python parse yields no fields for a row, bash produces a zero-length array, the body never runs, `bad` stays empty and the row is reported `✓ lobby row N … matches <cid>` — a green tick over nothing. The icp.yaml half of the same script guards this explicitly (`[ -n "$got" ] \|\| { echo "  ! …could not read…"; fail=1; }`); the lobby half has only a whole-registry `lobby_rows -eq 0` guard, and `--selftest` does not exercise the lobby half at all. **Latent, not live:** measured today the parser extracts 8/8 fields from all three rows. It goes live the first time the record shape, a field name, or the CLI's Candid spacing changes |
| [H-61](#h-61) | medium | OPEN | — | — | `tools/shots/test-burn-table.mjs`, `tools/cycles/burn-table.json`, `artifacts/cycles/*.json` | **the gate on the burn table conserves against the table's own internal state, which is this repository's standing lesson applied to cycles.** `test-burn-table.mjs` checks that every scenario recomputes from `burn-table.json`'s OWN measured and inherited components, and never compares any of them to the 13 raw runs in `artifacts/cycles/`. A hand-edited table that is internally consistent passes cleanly, and `scripts/cycles-runway.sh` reads `FALLBACK_BURN_PER_DAY` straight out of it with no verification, so the monitor's alarm floor can be loosened silently. Demonstrated: dividing every non-legacy `per_tab_per_day` by 4 and recomputing each scenario with the builder's own formula leaves the gate exit 0 (*"all 8 scenarios recompute from their own components"*) while the unknown-burn floor drops 2.0539 → 0.6075 T/day and the 51.4 T column goes 25 days → 84. The only outside anchor is read by `build-burn-table.mjs` and by nothing that gates |
| [H-62](#h-62) | medium | OPEN | — | — | `artifacts/cycles/fixed-{1,3,10}tab.json`, `tools/cycles/build-burn-table.mjs` | **the "typical" per-tab price was measured with the mechanism being priced switched off.** `artifacts/cycles/fixed-1tab.json`, `-3tab` and `-10tab` contain **no `check_timeouts` key at all** — the calls recorded are `heartbeat`, `get_balance`, `get_table_view` and `get_shuffle_proof` and nothing else — because the table used had 0 of 9 seats filled, and `clockIsDue` refuses both the predicate and the backstop on an empty table. That figure (0.0572 T/day) is then multiplied by 6 and 10 in the *"500 hands/day, N tabs open (typical)"* rows, which are the rows [E-55](#e-55)'s corrected runway table and wave 14's headline quote. **A table dealing 500 hands a day is by definition one where the nudger fires between every hand**, so the row is internally contradictory. Measured on an occupied table: ~1,963 calls/day/tab ≈ 0.0135 T/day, so the 10-tab typical row is ≈0.832 T/day (61 days on 51.4 T, 12 on 10 T) rather than 0.6974 (73 / 14). Direction: safe-looking. Same species as [E-92](#e-92) itself |
| [E-85](#e-85) | medium | OPEN | — | — (red on 2 of the 24 recorded shots) | `.sit-controls > button.control-btn.destructive` in `PokerTable.svelte` | **the Leave-table control renders 6–7 px outside a 1440 px window** (`51.2x28 at x=1396`, right edge 1447.2) with `documentElement.scrollWidth == 1440` and `<body>` at `overflow-x: hidden`, so its tail is off screen and cannot be scrolled to. Found by `tools/shots/lib/table-in-frame.mjs`, invisible to every other gate: `occlusion.mjs` compares element rectangles against each other, not against the frame. The wrapping fix costs 43 px of dock and drops the desktop felt 27.8% -> 23.2%; the structural fix is to move `.sit-controls` into the left dock cell (~435 px unused). [DESIGN-BAR §11.7](DESIGN-BAR.md) |
| [E-86](#e-86) | high | FIXED | 12 | `dev.sh shots` -> the `deposit` scene's chain-agreement check, now anchored on four LABELS; a copy edit that moves a figure is a MISSING-LABEL structural failure instead of a silent re-pointing | `tools/shots/lib/chain-agreement.mjs`, the `.minimum-notice` block | **THE EIGHTH CROSS-AGENT DEFECT: correct totals, wrong recipients, every invariant silent.** The deposit check read `.minimum-notice`'s numbers **in DOM order** and assigned them (minimum, fee, minimum + 2 fee). At `134550e` the copy grew a FOURTH figure and reordered the rest, so `nums[0]` became the ADDRESS minimum (30 000 e8s) compared against the canister's `min_deposit` (20 000), and `nums[1]` became the wallet-route minimum (20 000) compared against `icrc1_fee()` (10 000). Both deposit shots went red as *"screen is 1.500x the chain"* — **the screen was right, every figure on it was correct, and the comparison had been re-pointed at the wrong quantity by a copy edit.** The fourth figure was asserted by nothing at all and the token census reported it unaccounted for |
| [E-87](#e-87) | high | FIXED | 12 | `dev.sh shots-verdict` / `dev.sh hygiene` -> rule 3: every recorded problem on an acknowledged shot must be named by a `covers` string, and every `covers` string must still match something | `tools/shots/verdict-gate.mjs`; `artifacts/screens/acknowledged-reds.json` | **the red ledger acknowledged a SHOT, not a PROBLEM.** Once a (scene, viewport) was red for any reason, every LATER failure on the same shot was absorbed in silence. Not hypothetical: `table-in-frame.mjs`'s first real conviction ([E-85](#e-85), a control a player cannot reach) landed on two shots already acknowledged under [E-76](#e-76) for a felt shortfall of 0.2 points, and the gate stayed green |
| [E-88](#e-88) | medium | FIXED | 12 | `dev.sh shots` -> `buildVerdicts`; the vocabulary now carries every phrase the checks emit, and a new check must add its own | `tools/shots/lib/verdicts.mjs` `problemsOf` | the tracked verdict keeps clauses matching a six-word failure vocabulary and **DROPS the rest whenever at least one matches**. Neither the felt-area floor (*"below the 45% floor"*) nor `table-in-frame.mjs` (*"is NOT fully inside the 1440x900 frame"*) used any of the six. They survived only because they happened to be the ONLY clause on their shot; the first shot to fail an occlusion check **and** the felt floor would have recorded the occlusion and lost the felt out of the file the gate reads |
| [H-50](#h-50) | high | FIXED | 12 | `dev.sh archive` (and `make archive`), plus step 7 of 8 in `dev.sh test` | `tools/archive/**`, 39 offline cases | **[H-45](#h-45) again, in the wave that closed H-45.** `tools/archive` shipped with 39 self-tests, no `dev.sh` target, no make rule and no CI job — and [docs/ARCHIVE.md §8](ARCHIVE.md) said they held everything above, in a section that opens *"A gate nothing runs is worse than no gate"*. The tool is an independent reimplementation of [SHUFFLE-SPEC](SHUFFLE-SPEC.md) sharing no line with `poker_core` or the canister, so it is the only thing in the tree that can catch those two being wrong together |
| [H-51](#h-51) | high | FIXED | 12 | `dev.sh shots-selftest` -> `tools/shots/test-table-in-frame.mjs`, 17 cases; verified to convict by reverting the hardening (3 of 17 go red) | `tools/shots/lib/table-in-frame.mjs` | **the new gate had no gate, and its own headline conviction was unprotected.** `grep -rn table-in-frame` returned ONE hit, `run.mjs:41`, so the only runner needed a live replica — the condition [E-63](#e-63) blames for sitting red a whole wave. Worse: `.seat` is a `0x0` point on the ring, so the naive measurement is inside the frame by construction; restoring it left `foldTableInFrame` reporting *"9/9 seat pods, all in frame"* over zero-size boxes with every target green. Three blind spots are now asserted: the seats-block SHAPE, an empty pod set, and the board's silent skip (cross-checked against painted hole cards) |
| [H-52](#h-52) | high | FIXED | 12 | `dev.sh custody` -> `./scripts/check-candid.sh --guardian-only`, run BEFORE the tests; verified to go red by deleting one method from the committed `.did` | `scripts/check-candid.sh`; `src/guardian_canister/guardian_canister.did` | **the guardian's whole safety claim is "no destructive verb on its wire", and the three gates that assert it all parse the COMMITTED `.did` with nothing comparing that file to the module.** `check-candid.sh` covered `lobby`/`table`/`history` only, and `cmd_custody` ran `build-guardian.sh` **without** `--did`. A critic compiled a real `install_chunked_code(mode = Reinstall)` into the guardian, left the `.did` as committed, and all nine custody tests passed GREEN over a canister that can wipe a funded table |
| [H-53](#h-53) | high | FIXED | 12 | `dev.sh no-peeking` (and `make no-peeking`), plus step 8 of 8 in `dev.sh test` | `tests/no_peeking/**` (5 `[[test]]` targets, 36 tests), `src/no_peeking/**` | **[H-45](#h-45) a THIRD time, and this one names itself.** `tests/no_peeking/Cargo.toml` declares all five targets explicitly, with the comment *"Named explicitly for the reason every target in tests/money_safety/Cargo.toml is named explicitly: an auto-discovered target is a target no human ever types"* — and then no `dev.sh` target, no make rule and no CI job typed any of them. 36 tests, all green, ~30 s. Several are the [FINDING 23](SECURITY-FINDINGS.md#finding-23) pattern and PASS while a defect in the spike is live (`two_concurrent_try_advance_calls_pay_the_pot_out_twice`) |
| [T-44](#t-44) | medium | OPEN | — | — | `solvency.js` `readTableSolvency`, `cycleRunway.js` `readCycleRunway`, both rendered by `DepositModal.svelte` | **the two warnings on the screen money leaves from are UNCERTIFIED QUERY REPLIES, and their reassuring state renders NOTHING.** [FINDING 35](SECURITY-FINDINGS.md#finding-35)'s solvency verdict and [E-55](#e-55)'s runway notice are both read with a `query`: one replica answers out of its own memory and no consensus is involved. `SolvencyNotice` renders nothing at all when the verdict is `covered`, so a replica that answers `covered` (or that answers a shape the interpreter cannot read, which degrades to a quieter block) produces a deposit screen that is pixel-identical to a healthy table. The same replica can report a runway of years. Neither instrument is anchored outside the canister, so **the one screen that exists to tell a player not to deposit can be silenced by the cheapest attacker in the model**, and the silence looks like good news. Query-signature verification ([T-43](#t-43)) removes the gateway/MITM class but not this one: a signed query is still one replica's opinion. The fix is to read the pre-deposit verdict through an UPDATE call (consensus) or to state on screen that it is unverified |
| [T-43](#t-43) | high | FIXED | 14 | `cd tests/money_safety && cargo test --test deposit_trust_root -- the_agent_verifies_query_signatures` — verified RED on the pre-fix tree (`verifyQuerySignatures: false`) and GREEN after | `src/cleardeck_frontend/src/lib/canisters.js` `createAgent()` | **the app switched OFF the only check that binds a query reply to the subnet that answered it**, with the comment *"Disable query verification for now - there may be subnet key issues"*. `false` is not the library default; it had to be written. With it off, anything on the path — a boundary node, an HTTP gateway, a proxy, a compromised CDN edge — can rewrite ANY query reply, so [FINDING 42](SECURITY-FINDINGS.md#finding-42)'s substituted `get_tables()` needed no dishonest replica at all, only a position on the wire. Restored to `true` and MEASURED: the lobby list, the table view and the deposit modal all render on the local replica with **zero console errors** (`node tools/shots/repro-finding42.mjs`, `artifacts/finding42/evidence.json`). MAINNET NOT TESTED (hard rule 1) — smoke-test the lobby on the next deploy; the failure mode is a visible "Failed to load tables". **It is not the answer to FINDING 42:** a verified query is one replica's opinion, signed, not a consensus result |
| [T-42](#t-42) | high | OPEN | — | — | `tools/archive/lib/collusion.mjs` signal 2; `tools/archive/selftest/synthetic.mjs` | **the collusion detector accuses honest winning players and cannot tell them from cheats.** Signal 2's null (*"does A lose to B faster than A loses to everybody else"*) is ALSO the definition of *"B is better than A's other opponents"*, so it is false under ordinary skill variation and its error rate rises toward 1 with sample size rather than holding at alpha. Measured on **800 real, uncoordinated `table_3` hands**: two `REVIEW` rows at q=1.50e-4, both naming the best player — the same verdict, count and q as a scripted chip dump. Invisible to the shipped gate because `synthetic.mjs` has no per-player skill parameter, and its false-positive assertion tolerates `0.12` while claiming alpha `0.05`. The bootstrap p-value is separately anti-conservative (fires at 14% where it claims 5%). Disclosed at the top of [ARCHIVE.md §6](ARCHIVE.md) and in the tool's own output; **the fix is a skill-adjusted null, not a tighter alpha** |
| [H-54](#h-54) | high | FIXED | 12 | `dev.sh test` -> `with_timeout`, whose watchdog now writes to `/dev/null` and whose children are killed BEFORE it is. Before/after reproduced with a 1-second command and a 300-second bound: the old form blocks its consumer indefinitely, the new one returns at once and leaves no orphan | `scripts/dev.sh` `with_timeout` | **[H-42](#h-42)'s mechanism, and H-42 blames the wrong step.** `kill "$wd"` kills the watchdog SUBSHELL; the `sleep` it forked survives, orphaned to init, **still holding the stdout and stderr it inherited**. So `./scripts/dev.sh test` exits normally and any consumer of its output — `\| tail`, `\| tee`, `$( )`, a CI log collector — then blocks for the remainder of the bound with **zero CPU anywhere**, which is exactly what H-42 describes. Measured this wave: the gate finished and `sleep 900` sat at PPID 1 holding the pipe for another **14 minutes**. 900 + 300 is H-42's twenty; all of it is post-hoc, after every gate has already passed or failed. H-42 diagnosed the step BEFORE this one, for having no time bound; the cause is the two steps that DO have one |
| [E-89](#e-89) | high | FIXED | 14 | `./scripts/dev.sh fuzz-default` (seed `0xc1ea2dec0002` clean); `cd tests/money_safety && cargo test --test deposit_floor` — the six NEW tests, each verified RED against `git show HEAD:src/table_canister/src/lib.rs` in a `cp -Rc` tree | `tests/money_safety/src/invariants/reachability.rs` `check_drain`; `Currency::ICP` deposit-sweep floor | **`./scripts/dev.sh fuzz-default`, and therefore `./scripts/dev.sh test`, is RED — and the red is TRUE.** Seed `0xc1ea2dec0002`, twice, byte for byte: two players each send **10,001 e8s** to their own published deposit address, and after every legal player-side exit is driven to exhaustion the canister still owes **20,002 e8s that no player call can move**. 10,001 is inside [FINDING 31](SECURITY-FINDINGS.md#finding-31)'s dead band `(10_000, 20_000]`, and the drain transcript prints `sweepable=true` beside each one, which is FINDING 31's lie in the canister's own words. **This is the gate FINDING 31 never had**, in a five-op minimal reproducer. It fires at 20,002 rather than at 10,001 because `check_drain`'s `UNMOVABLE_DUST_E8S` is a PER-ACCOUNT floor (`min_withdrawal + fee`) compared against an AGGREGATE, so one stranded player is tolerated and two are not — the threshold's verdict depends on seat count rather than on whether the money is recoverable. **Pre-existing at `134550e`: no canister source and no fuzzer source changed in wave 12** |
| [E-92](#e-92) | high | FIXED | 14 | `node tools/shots/test-poll-updates.mjs` (discovered by `./scripts/dev.sh shots-selftest`, which `./scripts/dev.sh test` runs); the per-tab budget by `node tools/shots/test-clock-nudge.mjs`; the published numbers by `node tools/shots/test-burn-table.mjs` | `src/cleardeck_frontend/src/routes/+page.svelte` poll -> `check_timeouts`; every runway figure in the tree | **the render rate was driving an UPDATE loop, and it was the largest single cost of running this game.** `loadTableState` ran on `setInterval(…, 500)` and its FIRST statement was `await tableActor.check_timeouts()` — `#[ic_cdk::update]` in `lib.rs`, no `query` in `table_canister.did`. **MEASURED on the local replica against a no-tabs control (`tools/cycles/tab-burn.mjs`): one open browser tab cost 1.1380 T/day; ten tabs on one table burned 9.7618 T/day against an idle 0.0411.** At 500 hands/day that is **11.5 T/day, which is ZERO days on the 10 T [E-55](#e-55) headlines and four days on the 51 T the fixtures hold** — and the freezing reserve, nominally 30 days, was worth **four minutes**. No cycles figure in the tree counted any of it: six files carried a hand-copied table and all six priced a tab as a 10-second heartbeat. FIXED by splitting the loops — the 500 ms poll is queries only, and the clock is advanced by `$lib/clockNudge.js` only when a deadline is crossable, floored at 2 s and backing off to 30 s when calls change nothing. Re-measured: **0.0572 T/day per tab typical and 0.1929 T/day at the policy's ceiling — 19.9x and 5.9x cheaper**. Every runway figure now comes from one generated file, `tools/cycles/burn-table.json` |
| [E-84](#e-84) | fund-theft | OPEN | — | — | the CONTROLLER SEAT: `icp.yaml`, every fund-holding canister on mainnet | **the id [FINDING 23](SECURITY-FINDINGS.md#finding-23) never had, so it could be scheduled.** One controller principal per canister, and a controller can call `install_code --mode reinstall` or `uninstall_code` on a funded table: measured at **40.00000000 ICP of player claims destroyed by one command with the same wasm and no code change**, ledger untouched, no restore path. No in-canister check can reach it — `require_controller()` lives in the table, these are calls to the MANAGEMENT canister. `src/guardian_canister/` is the mitigation, is built and gated (`./scripts/dev.sh custody`, 9 tests) and is **NOT DEPLOYED**, so nothing has changed for a real deposit. Disclosed as of wave 12 ([D-13](#d-13)) |
| [E-41](#e-41) | high | FIXED | 13 | `dev.sh test` step 4 -> `cargo test --test regressions` -> `reg39_a_settled_hand_is_never_settled_a_second_time` (the door AND the money, both legs verified red on the unfixed build: `use_time_bank -> Ok(0)`, then `created 4000000 e8s of chips out of nothing`), plus `dev.sh test` step 1 -> `cargo test --workspace` -> `payout_tests::e41_*`, three tests for the three guards, each verified to go red when **its own guard alone** is removed and green under the other two | `use_time_bank` (the door), `resolve_expired_action_timer` (the clock), `advance_game` (the settlement path), `end_hand_single_winner`/`determine_winners` (the money) — all `src/table_canister/src/lib.rs` | **the canister owed 4,000,000 e8s it did not hold, and the mechanism is A HAND SETTLING TWICE.** `finish_hand` empties `state.pot` but not `total_bet_this_hand` (only `start_new_hand` clears that), so between hands the table sits on a COMPLETE PAYOUT BASIS over an EMPTY POT. `use_time_bank` asked only "is `action_on` pointing at you" and never "is there a hand", so it armed an `ActionTimer` on a finished hand; 30 s later the clock folded a seat out of a hand already paid, `advance_game` read `count_active_players == 1` off last hand's cards, and `end_hand_single_winner` paid the whole basis again. `plan.conserves()` is TRUE throughout — awarded equals the plan's own `collected` — so `apply_payouts` never trapped. Shrunk from 400 steps x 2 seeds to **one limped heads-up hand and five ordinary player calls**. [FINDING 39](SECURITY-FINDINGS.md#finding-39) |
| [E-55](#e-55) | high | OPEN | — | — (the row stays gateless because the DEFECT is still live: nothing funds a table. The measurement and monitoring halves that closed are held by `dev.sh test` → `cycles_runway`, `dev.sh shots-selftest` → `test-cycle-runway.mjs`, `test-poll-updates.mjs`, `test-clock-nudge.mjs` and `test-burn-table.mjs`, and `scripts/cycles-runway.sh --selftest`) | cycles: measured properly at last, monitored, and shown to players; still nothing tops it up | **a canister below its freezing threshold rejects every update call at once** — `deposit`, `withdraw`, `cash_out`, `player_action`, `abandon_stuck_hand` — which is total custody failure with no attacker. **The 226-day figure was for a table nobody was using; the 20-day figure that replaced it was still wrong, because it priced an open browser tab as a heartbeat stream.** Measured against a no-tabs control in wave 14 ([E-92](#e-92)): **a table dealing 500 hands a day with ten browser tabs open burned 11.5 T/day — ZERO days on 10 T and four days on the 51 T these canisters hold**, with everybody behaving normally, and the freezing reserve was worth four minutes. The poll is fixed and the same table is now 4–14 days on 10 T. **Days, not months. And nothing tops it up.** Also fixed earlier: `runway_days` was computed from a lifetime burn average, so a table that sat idle and then got busy over-reported its runway by 2.1x. Now warned to a player before they deposit, and read twice a day by a CI job that holds no credential |
| [E-91](#e-91) | high | FIXED | 13 | `dev.sh test` step 1 -> `cargo test --workspace` -> `history_canister::retention_tests::a_table_may_not_record_a_hand_naming_another_table` (+ `a_table_may_record_its_own_hand`, `the_admin_is_still_allowed_to_record_for_a_table`, `a_stranger_is_still_refused_before_any_of_this`, and `two_records_sharing_one_name_collapse_to_one_which_is_why_the_binding_exists`, which measures the mechanism the binding protects). Verified to go red: neutering the binding to `if false` fails exactly that one test, 22 of 23 still green | `history_canister` `record_hand` -> `may_record` | **THE NINTH CROSS-AGENT DEFECT, and it is a REGRESSION THIS WAVE INTRODUCED.** [E-71](#e-71) narrowed the de-duplication key from `(table_id, hand_number, seed_hash)` to the hand's NAME, `(table_id, seed_hash)` — and `record_hand` authorised the CALLER but never bound `record.table_id` to it, so a writer picked half of the name. The other half, the shuffle commitment, is PUBLIC: it is on the player's screen while the hand is running. Before the narrowing a forged record naming another table landed as a visible EXTRA record under its own `hand_number`; after it, the forgery and the genuine hand share one whole name, so the second to arrive is absorbed as a "retry" and **DISCARDED while its sender is told `Ok` with somebody else's id**. File the forgery first and a genuine hand is deleted from the permanent, append-only, "provably fair" archive, silently. That is [E-49](#e-49) exactly, in the wave whose own comments cite E-49 as the thing being avoided, reachable by any of the four authorised tables. **Every instrument built this wave reads green over it, because the failure is an ABSENT record rather than a wrong one.** Closed by binding `record.table_id` to the caller; the admin stays exempt and it is [SAID](SECURITY-FINDINGS.md#finding-23) rather than assumed, because the admin can reinstall this canister anyway |
| [E-71](#e-71) | high | FIXED | 13 | `dev.sh shots-selftest` -> `tools/shots/test-hand-identity.mjs` (25 cases, no replica), whose case 4 injects the by-`hand_number` join and requires `JOIN BROKEN`; `dev.sh test` -> `cargo test -p history_canister` -> `eleven_hands_under_one_number_have_eleven_different_names` + 6 more; on a running canister, `history get_archive_integrity` (`name_collisions` and `records_without_a_usable_commitment` must be 0, recounted from the RECORDS not the index). Verified to go red end to end: the join reverted to `hand_number` turns the `handhistory` shot red naming both hands | `history_canister` `hand_uid`; `tools/shots/lib/chain-agreement.mjs` `joinTableHandsToArchive` | the archive's `(table_id, hand_number)` was not a key: measured, **947 of 1,651 citations named more than one record, 2,514 of 3,218 records (78%) lived under one, and "table_2 hand 1" answered to 70 records with 6 different pots**. A hand is now named by what is intrinsic to it — `table_id:seed_hash`, the shuffle commitment, unique over all 3,218 existing records — DERIVED on read, so every record already stored was named without being rewritten, renumbered or reindexed (proved by an upgrade over the live 3,218: 0 lost, 0 added, 0 changed). Old citations still resolve, via `resolve_hand_number`, to the labelled SET they always named |
| [H-23](#h-23) | high | FIXED | 13 | CI `fund-safety-fast` (**declared on every PR; NOT a required status check — see the correction below**) → `./scripts/ci-fund-safety.sh fast`: 21 suite invocations including the money invariants, `deposit_replay`, the custody gate, the solvency gate and the settlement oracle. CI `fund-safety-deep` (nightly + every push to `main`) → `ci-fund-safety.sh deep`: the 9×600 fuzz, the fuzzer at its own defaults, the 138-state stall sweep, the clock, the runway, the full oracle sweep. CI `suite-wiring` → `./scripts/check-suite-wiring.sh`, which fails the build when any cargo test target is run by no tier, and which `--selftest`s its own ability to go red BEFORE it judges. **Verified to convict:** a 1% skim planted in `apply_payouts`, invisible to conservation, `sum(winners) == collected` and `total_liability()`, turns the fast tier RED. **CORRECTION, wave-13 reconciliation, verified against the API on 2026-08-09: nothing here is REQUIRED and the deep workflow has never run.** `gh api repos/JoshDFN/cleardeck/branches/main/protection/required_status_checks` -> `404 Required status checks not enabled`; `.../rulesets` -> `[]`; `.../actions/workflows` lists CI, Cycles monitor, Deploy to IC mainnet, Deployed drift and Security, and **`Fund safety (deep tier)` is not among them**. So a red fund-safety job blocks no merge, and the register said REQUIRED where the repository says nothing is. The job exists, is runnable by a human as `./scripts/ci-fund-safety.sh fast`, and is proved to convict a real theft — **turning it on in branch protection is one click nobody has made, and it is the single highest-value item left in this row**. **WAVE-14 ADDITION:** `check-suite-wiring.sh` is no longer CI-only — it is step **[1/9]** of `./scripts/dev.sh test` and it `die`s, so the one check that can catch a suite nobody runs now runs for a developer too, which matters precisely because nothing here is required ([H-45](#h-45)). The `fast` tier also grew `solvency_definition` and `deposit_trust_root` this wave | `.github/workflows/ci.yml`, `.github/workflows/fund-safety-deep.yml`, `scripts/ci-fund-safety.sh`, `scripts/pocket-ic.sh`, `scripts/test-suites.list`, `scripts/check-suite-wiring.sh` | **every fund-safety result in this project's documents came from a harness no CI job invoked.** `cargo test --locked --workspace` could not reach one of them: the three PocketIC harnesses carry a bare `[workspace]` on purpose, so the deployed canisters stay byte-reproducible, and that put every money invariant outside the only test command CI ran. The wiring check caught **three unwired suites while it was being written**, including its own blind spot — `--test X` does not run `--lib`, so **twelve unit tests**, one of them inside the money-safety harness itself, were run by nothing |
| [H-26](#h-26) | high | FIXED | 14 | `cd tests/money_safety && cargo test --test fuzz -- a_seeds_table_shape_does_not_depend_on_where_it_appears_in_the_list` — milliseconds, no replica, in the same binary as the fuzzer it guards and therefore in `dev.sh test`, the `fast` CI tier and `fuzz-default`. **Proved RED on the pre-fix body** (`run_shape` reverted to `position % 3`): *`seed 0x0 plays a DIFFERENT GAME at position 1 than at position 0: heads_up_icp vs six_max_icp`*, then green on the fix. **And anchored OUTSIDE the code:** seed `212967420072194` at 400 steps was run twice in two separate processes, once after `212967420072193` and once alone, and the two `money-fuzz-report.json` run objects are byte-identical (`config`, `steps_executed` 400, `hands_completed` 7, `upgrades` 6, `final_ledger_main`, `final_internal_total`, `transcript_tail`) | `tests/money_safety/src/fuzz.rs` `run_sequence` | a fuzz run is documented as "a pure function of `(seed, config, actor_names, steps)`" and is not. Seed `212967420072194` at 400 steps plays **4 hands and finds nothing alone, 5 hands and two fund-creation findings when seed `212967420072193` ran before it in the same process**. Every `MONEY_FUZZ_SEEDS=<one seed>` reproducer in this repo is therefore unverified. **The COVERAGE half is closed in wave 13 and the purity half is not.** Two changes, both needed, neither about run length: (1) `Op::UseTimeBankOnClock` + `under_the_action_pointer`, because `Op::UseTimeBank` names a RANDOM actor and the harness's only seat-resolver `on_the_clock` returns `None` unless a hand is live, so no op in the alphabet could call a between-hands surface with the right caller; (2) a state-arming op now sometimes carries its own `AdvanceTime(31 s)` + `CheckTimeouts`, because every clock defect here is "one message arms it, a later deadline acts on it" and the generator was drawing the pair by luck. **Measured, all at the default 3 seeds x 220 steps with nothing in the environment:** HEAD's generator finds 1 finding (E-89, 20,002 e8s, 74.0 s); change (1) alone still misses E-41; (1)+(2) on the UNFIXED canister finds **32**, including `M1_CONSERVATION delta=-200000000 / -399000000 / -601500000` and the `CRITICAL: pot accounting disagreement` line, in 87.8 s; on the fixed canister it is back to 2 (both [E-89](#e-89) family) in **55.6 s**. Seed order still decides what a run explores. **TWO CORRECTIONS, wave-13 reconciliation.** (1) *The 55.6 s figure is for a configuration nothing runs.* It was measured with `MONEY_FUZZ_SHRINK=0`; `dev.sh test` and `make fuzz-default` both `env -u` that variable, so the shrink budget is the default 60 and a run that finds anything spends ~200 s per shrink pass. The generator change is not free: measured at the real default, the new generator cost **597.6 s against HEAD's 389.5 s** because it produced two shrink passes rather than one. (2) *The second of the two surviving reds was not [E-89](#e-89).* It was `M3_NO_RAKE +10000` at seed `0xc1ea2dec0001` — [FINDING 44](SECURITY-FINDINGS.md#finding-44), a FALSE conviction caused by M3's window guard reading `ledger_main` while `internal_total()` counts deposit subaccounts. Fixed in this pass, and with it seed `0xc1ea2dec0001` is clean in 19.9 s and `fuzz-default` is red on E-89 alone |
| [H-42](#h-42) | high | FIXED | 14 | `scripts/dev.sh` `with_timeout`, now used on **every one of the nine steps** of `cmd_test` with the bound printed next to the measured time. **H-54's fix was vacuous and this is the measurement:** at HEAD, `out="$(with_timeout 2 sh -c 'sleep 60; echo never')"` **returned after 60 s**, because `kill -9 "$pid"` reaches the subshell only and every process it forked keeps the inherited stdout. After the fix the same command returns in **3 s with rc=124**, a deeper tree (`sh -> sh -> sleep 90`) returns in **2 s**, `ps` shows no survivor, and a command that finishes inside its bound still returns its own exit code (7 stays 7) | `scripts/dev.sh` `with_timeout`, `descendants`, `kill_tree`, `timed_step`, `cmd_test` | **`./scripts/dev.sh test`, the repo's primary gate, hung for 33 minutes** with zero CPU on both the test binary and its own PocketIC. The money-safety targets have NO time bound; the settlement targets one step later have two. Every leg is green when run directly (`invariants` 45/0 in 63 s single-threaded) |
| [D-11](#d-11) | high | FIXED | 14 | `./scripts/check-declarations-js.sh` (CI job *Candid interface drift*, and step [1/9] of `dev.sh test`): regenerates every `<n>.did.js` and `<n>.did.d.ts` from the committed `.did` with the same `candid_parser` bindings `didc bind` uses, and diffs. `--selftest` deletes `get_tables` from a binding and requires a conviction; `--write` is the only sanctioned way to change these files. **Proved RED on the unfixed tree: 10 of 10 files drifted**, then green after `--write`, and the app was re-opened in a plain browser afterwards to confirm the regenerated bindings still drive it | `src/declarations/<n>/<n>.did.js` via `src/lib/canisters.js` | the frontend's Candid bindings are a **third** copy of the interface and have drifted from both. `table_1.did.js` does not declare `get_solvency`, `get_all_ledger_intents`, `get_cycle_status`, `refresh_solvency` or `get_deposit_replay_state`, so the custody and solvency instruments built in waves 7–10 cannot reach the UI. Same shape as [E-67](#e-67) |
| [H-45](#h-45) | high | FIXED | 14 | step **[1/9]** of `./scripts/dev.sh test` -> `./scripts/check-suite-wiring.sh`, and it is `die`, not a collected failure: a tree containing a test target nothing names does not get to run its gates. **Proved RED twice. Once PLANTED:** an empty `tests/money_safety/tests/zz_planted_unwired.rs` turns `dev.sh test` red in **8 seconds** with `FATAL: suite wiring is broken -- a test target in this tree is run by nothing`. **Once FOR REAL, unplanted, during this wave:** `tests/money_safety/tests/solvency_definition.rs` (FINDING 43/38) was written by another owner and named by nothing, and this step said so within a second instead of a human finding it a wave later — H-45's SEVENTH recurrence, now wired. Second half of the same fix: `money_safety_fast_subset` no longer `&&`-chains its targets, so one red target cannot skip the fifteen after it | `tests/money_safety/tests/{stall_agreement,solvency,deposit_subaccount_anchor,fund_reachability,oldest_cluster,deposit_surface}.rs` | **[H-17](#h-17) AT FOUR TIMES THE SCALE, and it is holding up the two most recent cross-agent defects.** `scripts/dev.sh cmd_test` names its money-safety targets one by one *precisely so* a cargo-auto-discovered target cannot be a target nobody runs — and four targets are not on the list, are in no make target, and are in no CI job. They are the named gates of [E-59](#e-59)/[FINDING 25](SECURITY-FINDINGS.md#finding-25) (M13 ONE BELIEF, the FOURTH cross-agent defect), [E-70](#e-70)/[E-72](#e-72)/[FINDING 35](SECURITY-FINDINGS.md#finding-35)/[36](SECURITY-FINDINGS.md#finding-36) (the FIFTH), [FINDING 28](SECURITY-FINDINGS.md#finding-28)/[FINDING 11](SECURITY-FINDINGS.md#finding-11)/[E-12](#e-12), and M9's own file. **46 tests, and the count grew by two files DURING this pass** — `oldest_cluster` (the gate the [E-78](#e-78) row names) and `deposit_surface` were both added unwired by other owners in the same wave. **Three of the six are wired as of 2026-08-06:** `oldest_cluster`, `deposit_surface` and `deposit_subaccount_anchor` are now named in `scripts/dev.sh cmd_test` AND in `tests/money_safety/Cargo.toml`, by the owners who filed them. `deposit_subaccount_anchor` mattered most: it was the ONLY gate on [E-12](#e-12)/[FINDING 11](SECURITY-FINDINGS.md#finding-11) and [FINDING 28](SECURITY-FINDINGS.md#finding-28), eleven tests, run by nothing since wave 8. **`stall_agreement`, `solvency` and `fund_reachability` are still unwired**, and [H-48](#h-48) says the last of those three is RED. Verified 2026-08-06 by grepping every `--test <name>` in `scripts/`, `Makefile` and `.github/` |
| [H-48](#h-48) | high | FIXED | 14 | `cd tests/money_safety && CLEARDECK_TABLE_WASM=… cargo test --test fund_reachability -- --test-threads=1`, named in `dev.sh test` step [5/9] and in the `fast` CI tier. **Settled by running it, not by reading a report: 6 passed, 0 failed, 13.04 s**, against wasm `3890a6d4a1861343df40c08772979171bc15fed8c51102c81f3c849346b29a0d`. **And the question the entry actually asked is answered: the GATE was stale, the code was right.** `git log -p` on the file shows the two assertions were rewritten in commit `134550e` from a single `advance(30+300+5 s)` to `12 x advance(60 s)`, with the reason in the file (*"a single `advance(335s)` gives the canister nothing to witness"* — the E-59 predicate counts committed messages, not wall clock) and the resulting coverage loss recorded in the doc comment rather than hidden. The file also carries `m9s_per_step_check_can_actually_fail`, so it is not a gate that cannot go red | `tests/money_safety/tests/fund_reachability.rs:266,:349` (**wave 12: re-run at HEAD, 6 of 6 PASS**) | **M9's own file is RED and no target runs it.** 2 of its 6 tests fail on the wasm `dev.sh test` just built, both at the predicate [E-59](#e-59) changed from the wall clock to ATTEMPTS. Probably a stale gate — `timers::the_table_settles_itself_with_no_external_caller` is green and proves the money is reachable on a subnet — but nothing anybody runs has had to answer for it. See [H-45](#h-45) |
| [L-03](#l-03) | high | OPEN | — | — | `.alpha-warning-banner` + `header` in portrait | the phone lobby cannot reach the reference band from `Lobby.svelte` at all: with the lobby's own furniture at **zero** the first card is still at **45.9%**. The 28 px needed are chrome, and the mechanism to release them already ships one condition away |
| [L-04](#l-04) | high | FIXED | 14 | `./scripts/check-deployed-config.sh` (daily CI job *Deployed drift*), which now reads `lobby.get_tables()` and diffs **every registered row against its own table contract**, field by field, plus a rule that a registered NAME may not quote a price at all. **Proved RED with real drift and healed:** a name put back to `"6-Max - 0.05/0.10"` and the table_2 contract's clock moved 45 -> 60 produced *`small_blind: lobby advertises 1000000, the contract charges 5000000`* and four more lines plus the name rule; after `refresh_all_table_configs` and restoring the contract it reads *all 4 table config(s) match icp.yaml, and every lobby row matches its contract*. **The missing method exists**: `refresh_table_config` / `refresh_all_table_configs` COPY the config out of `get_table_view()` rather than taking one as an argument, so the registry cannot be told a figure the contract does not charge. LOCAL ONLY; mainnet is untouched and still needs the same one call | lobby canister registry, `src/lobby_canister/src/lib.rs`, `scripts/check-deployed-config.sh` | the lobby's registered *names* quote blinds the table contracts do not charge (10× and 5× wrong), and its registered *configs* disagree with the contracts on four fields each. **This, and only this, is why both lobby scenes are red.** Every price the client computes is the contract's |
| [H-55](#h-55) | high | FIXED | 14 | `./scripts/check-deployed-config.sh --selftest`, which asserts the extractor can read all eight `TableConfig` fields out of a real Candid reply, that an ABSENT field reads as empty rather than as `0`, and that a 5x drift does not compare equal. It runs before the script judges anything, in the daily *Deployed drift* CI job. **Proved RED before the fix by the guard's own subject:** table_2's contract was moved to `action_timeout_secs = 60` against icp.yaml's 45 and the script printed `✓ table_2 … matches icp.yaml`; after the fix the same tree prints `✗ table_2 … action_timeout_secs: declared 45, running 60` | `scripts/check-deployed-config.sh` field extractor | **the guard against declared-vs-deployed drift compared ONE field out of EIGHT and said "matches".** Its extractor was `grep -oE '[0-9_]+' \| head -1 \| tr -d '_'`, and `[0-9_]+` matches **the underscore inside the field name**: on `small_blind = 5_000_000 : nat64` the first match is the `_` in `small_blind`, `head -1` takes it, `tr -d '_'` empties it and the caller's `[ -n "$got" ] \|\| continue` SKIPPED THE FIELD. Seven of the eight names in `TableConfig` carry an underscore, so the only field ever compared was `ante`. This is the script written in wave 13 to stop [D-12](#d-12)'s `btc_table_1` running at one tenth of its declared stakes for its whole life, it would not have caught that either, and `.github/workflows/deployed-drift.yml` has been running it against MAINNET daily and reporting green |
| [T-45](#t-45) | medium | OPEN | — | — | `src/cleardeck_frontend/src/lib/oisy.js:60,:165,:301` | [T-14](#t-14)/[T-03](#t-03) surviving in a third place. `ic-config.js` and `auth.js` both derive the local gateway from `LOCAL_GATEWAY_PORT` now; the OISY wallet path still writes `isMainnet() ? IC_HOST : 'http://localhost:4943'` three times, so connecting an external wallet locally still points at a port this project does not run. It is in the built bundle today. Not fixed here: `src/cleardeck_frontend/**` is another owner's tree |
| [T-46](#t-46) | high | OPEN | — | — | `src/cleardeck_frontend/build/verify-bundle.mjs` `compiledNetwork()` vs `src/lib/ic-config.js` `buildValue()` | **`npm run build:mainnet` is RED at HEAD, and it is the gate [E-95](#e-95) put on CI's `Build frontend` job.** `compiledNetwork()` in the verifier matches `function X(){return"ic"}` in the minified bundle; wave 14's `buildValue(() => import.meta.env.VITE_ICP_NETWORK)` refactor wrapped the substitution in a closure, so the emitted function no longer has that shape and the verifier reports *"no network literal compiled in: the bundle would fall back to sniffing window.location.hostname"* and exits 1. The value is almost certainly still correct — vite's `define` substitutes inside an arrow function — so this is the DETECTOR going stale, which is the worse of the two possibilities: the one gate on T-01 now fails on every build and the pressure is to switch it off |
| [T-14](#t-14) | high | FIXED | 14 | `assertBundleLocalGateway(GATEWAY_PORT)` in `tools/shots/lib/frontend-build.mjs`, called by `buildFrontend` — so it runs on `dev.sh local-up` and on every screenshot sweep. **Proved RED by building with the PRE-FIX environment** (`buildEnvFor` minus the two keys this fix adds): *`ABORT: the built bundle … contains no reference to the local gateway port 8077`*; green on the fixed build (`port 8077 found in 1 built file(s)`). **And verified on rendered pixels through a plain browser with no shim:** `http://5uljf-…-cai.localhost:8077/` renders *3 tables · 0/17 seats · 0% rake* with no console error, where before it showed a raw fetch stack trace and "The lobby canister is reporting no tables" | `tools/shots/lib/frontend-build.mjs` `buildEnvFor`, `tools/shots/run.mjs`, `tools/shots/perf.mjs` | the deployed local frontend points its agent at **127.0.0.1:4943** while the gateway is on 8077, so the app only works behind the screenshot harness's own shim. Opened in a plain browser it shows a raw fetch stack trace and "The lobby canister is reporting no tables" |
| [D-03](#d-03) | medium | OPEN | — | — | every component `<style>` block | 16 border radii, 28 font sizes, 9 greens, 6 ambers, 11 greys, 8 panel tints, 8 panel strokes; four buttons in one header row with three heights, two radii, two font sizes and two accent families |
| [D-06](#d-06) | medium | OPEN | — | — | `README.md` lines 95-125, `docs/SHUFFLE-SPEC.md` | two claims a stranger reads as stronger than they are: *"the commitment is published before the deal"* (true only inside a single message, `start_new_hand` commits and deals atomically, so no outsider can observe the commitment before cards exist), and *"You can verify that the deployed canisters match this source code"* (addressed to people who by construction cannot run the procedure) |
| [D-07](#d-07) | medium | OPEN | — | — | `README.md` §Verify the Code, the "⚠️ The mainnet canisters do not satisfy this yet" box | two trust surfaces of the same product make opposite claims about the same fact, which is [T-21](#t-21) again on the page a stranger reads first. The README box says the deployed mainnet modules **predate** the reproducible pipeline, carry no `git:revision`, and that a verifier should expect `NOT VERIFIED`. The wave-11 handover states all six backend canisters and the frontend run this tree and the hashes match a reproducible build **6 of 6**. One of the two is false and the README is the one a player reads |
| [E-08](#e-08) | medium | FIXED | 11 | CI `candid-check` → `./scripts/check-candid.sh --declarations` (self-tests that it can go red before it judges) | `src/table_canister/table_canister.did` | the published Candid does not describe the deployed code (241 diff lines) |
| [E-10](#e-10) | medium | OPEN | — | — | `PENDING_WITHDRAWALS`, `LAST_WITHDRAWAL` | not in `PersistentState`, so the withdrawal cooldown resets on every upgrade |
| [E-33](#e-33) | medium | OPEN | — | — | `join_table` / `start_new_hand` | no post-or-wait-for-the-big-blind rule, so a player can cycle in and out taking free non-blind hands |
| [E-34](#e-34) | medium | OPEN | — | — | `start_new_hand` blind assignment | no dead-button rule: when a seat between the button and the blinds empties, a player is skipped for the big blind |
| [E-50](#e-50) | medium | OPEN | — | — | `scripts/dev.sh` `up_wire` | `icp canister call` **exits 0 when the method returns `variant { Err }`**, and every controller-only call in the script relied on the exit code. That, plus `$CONTROLLER` not actually being the controller, is why T-34 held for a whole wave with the script printing success |
| [E-65](#e-65) | medium | OPEN | — | — (red on 4 of the 24 recorded shots) | the lobby canister's registered table names; `LobbyTable` rows | every lobby row's NAME quotes `0.01/0.02` while the canister's config says `0.05/0.10` and `0.10/0.20`, so the lobby advertises stakes **5x and 10x below** what a player is charged on sitting down. Red on 4 of the 24 recorded shots |
| [E-67](#e-67) | medium | OPEN | — | — | `src/declarations/history/history.did.js` `idlFactory` | the archive's generated JS bindings are stale, so `dealt_in`, `contributed`, `left_mid_hand` and `dealt_in_count` are silently dropped by the decoder. Candid record subtyping means **nothing errors**: the hand-history view simply cannot show a player how many people were dealt into their hand, which is the number they need to check it |
| [E-68](#e-68) | medium | OPEN | — | — | `UNRECORDED_HANDS`, a bare `thread_local` absent from `PersistentState` | an upgrade destroys every hand the archive has not acknowledged yet — up to 64 complete records, in exactly the state the backlog exists for — and `get_history_status` then honestly reports a backlog of zero |
| [E-74](#e-74) | high | OPEN | — | — | `scripts/dev.sh` `up_wire` → `lobby set_admin` / `init_microstakes_tables` | `local-up` reported success for a lobby it did not populate, and an empty lobby makes the **entire** screenshot harness unrunnable — which is why the rendered notice gate was dark for two waves. `icp canister call` exits 0 on `variant { Err }` ([E-50](#e-50)). **HIT AGAIN 2026-08-06 and now diagnosed:** `local-up` printed `✓ lobby lists 0 table record(s)`, exited 0, and all 24 screenshot scenes failed with *"Lobby has no registered name for table_N"*. Root cause: `set_admin` is called AS `$CONTROLLER`, but the lobby's admin is whoever initialised it first; a different identity there makes `set_admin` refuse *"Only current admin can set new admin"* and `init_microstakes_tables` then refuse *"Only admin can initialize tables"*, both silently. **The silent half is closed** — `up_wire` now reads the count back and DIES, printing `get_admin` and the hand-over command. The bootstrap itself is still fragile and is why this stays open. Raised `medium` → `high`: it takes the project's only rendered-pixel gate offline without a word |
| [E-75](#e-75) | medium | OPEN | — | — (red on 1 of the 24 recorded shots) | `span.pod-clock` over `span.chips` in `PokerTable.svelte` | a player's stack figure is painted over by the pod clock at 390x844: **9.8% of "50.00" covered**. Same class as [E-63](#e-63), different pair of elements. Measured on rendered pixels |
| [E-76](#e-76) | medium | OPEN | — | — (red on 3 of the 24 recorded shots) | `felt-area.mjs` floors vs the 6-pod and 9-pod layouts | the felt drops below its floor on three of the twenty-four shots: 27.8% against a 28% floor on two desktop table scenes, 42.8% against 45% on `table-allin` mobile. Marginal and real |
| [E-77](#e-77) | medium | OPEN | — | — | `notify_deposit` (archived-block branch) | **renumbered from a second E-45 in wave 11; the id collided with the currency-guard defect.** A deposit made by plain transfer can only be credited while its block is still resident in the ledger canister. Once archived it can **never** be claimed, and `admin_restore_balance` was deliberately removed, so nothing can credit that user afterwards. The README documents this path |
| [E-80](#e-80) | medium | OPEN | — | — | `notify_deposit` destination branch, `src/table_canister/src/lib.rs` | the refusal asserts a negative the canister cannot know: *"Transfer was not to an account of this canister"* is returned for **any** destination that is neither the main account nor the CALLER's own deposit address — including another player's deposit subaccount, which is exactly where [FINDING 40](SECURITY-FINDINGS.md#finding-40)'s theft lands the money. The same sentence then says *"Check the destination against `get_deposit_address()`"*, which since [FINDING 34](SECURITY-FINDINGS.md#finding-34) returns the caller's OWN address and not the main account this branch is about |
| [E-81](#e-81) | medium | FIXED | 14 | `./scripts/dev.sh test` step 4 → `cargo test --test deposit_floor` → `a_balance_at_the_ledger_fee_is_refused_as_arithmetic_and_not_as_a_policy_minimum`, verified RED against `git show HEAD:src/table_canister/src/lib.rs` | `withdraw` minimum refusal, `src/table_canister/src/lib.rs` | the refusal states a universal guarantee the same sentence disproves: *"Your whole remaining balance can always be withdrawn in one call whatever its size, as long as it is more than the 0.0001 ICP network fee -- you have 0.0001 ICP."* It is the exact message shown to the player stranded by [FINDING 31](SECURITY-FINDINGS.md#finding-31), and it reads as a formatting mistake to retry rather than a door that will never open |
| [E-82](#e-82) | medium | OPEN | — | — | `notify_deposit` ICRC-2 branch, `src/table_canister/src/lib.rs` | on a canister whose stable state has been wiped ([FINDING 23](SECURITY-FINDINGS.md#finding-23)) the refusal says the block *"was credited to your balance when the pull happened and cannot be credited again"*. The replay refusal is right; the stated reason is a claim about the books that is false in that state, and it is the last door a wiped-out player tries |
| [H-11](#h-11) | medium | OPEN | — | — | `tests/money_safety/src/table_api.rs:80-108` | the fuzzer only ever runs a 30 s action timeout at `table_1` stakes; `table_2`/`table_3` shapes are never exercised |
| [H-22](#h-22) | medium | OPEN | — | — (contained by `with_timeout 900`, not fixed) | `tests/settlement` re-deal search | the exact-deal search has no attempt cap, so a deck that stops varying HANGS the suite instead of failing it; contained by a timeout in `cmd_test`, not fixed |
| [H-25](#h-25) | medium | OPEN | — | — | `./scripts/dev.sh known-defects` | one marker, for one low-severity defect. Twelve open engine defects in this register have none |
| [H-27](#h-27) | medium | OPEN | — | — | `tests/money_safety`, `tests/settlement` | what the widened attribution gate still does NOT reach, named per planted bug: swapped winner amounts need two payouts to two different people in one hand, which no ordinary-hand fixture and no 300-step fuzz seed produced; `push_winner`'s owner-merge needs a chair with two owners, which only the hand-written fixture builds |
| [H-29](#h-29) | medium | OPEN | — | — | both harnesses' `sha256` identity claim | the module hash is NOT a function of the source. Two trees with byte-identical source, the same pinned toolchain and the same `Cargo.lock` produced two modules that agree for all 2,053,251 bytes of code and data and differ only inside the semantics-free `name` custom section |
| [H-30](#h-30) | medium | OPEN | — | — | `ShuffleProof.svelte` placement + `shuffleproof` scene | the green verdict is **below the fold at both viewports**; on mobile it is inside a nested scroller (`.proof-sidebar`, `clientHeight 918`, `scrollHeight 2708`) that scrolling the page cannot reach |
| [H-43](#h-43) | medium | OPEN | — | — | `capture.mjs` `writeManifest` / `writeIndex` | a partial screenshot run **overwrites the full run's `INDEX.md` and `manifest.json`** with only the scenes it ran, in both `<sha>/` and `latest/`. The PNGs survive; the record of what they prove does not, and nothing warns |
| [H-44](#h-44) | medium | OPEN | — | — | `money_safety::fuzz::next_op` | the fuzz GENERATOR cannot produce the E-36 / FINDING 17 sequence. `JoinTable` + `SitIn` exist as ops, but the sequence needs a LIVE hand, an EMPTY chair and both calls in order, and 220 hostile steps at three seeds produced it zero times. M11 OUTCOME fires on the deterministic reproducer and is silent on every fuzz seed with the defect deliberately restored -- so the class is gated by the scripted probe, not by search |
| [H-46](#h-46) | medium | OPEN | — | — | `src/cleardeck_frontend/build/verify-bundle.selftest.mjs` | the wave-9 mainnet-bundle verifier's **mutation self-test** — 8 planted mutations, the thing that exists because the first version of that verifier passed 12 of 12 while measuring nothing — is invoked by no npm script, no make target and no CI job. `src/cleardeck_frontend/package.json` has `verify:bundle` and `verify:deployed` and no `verify:bundle:selftest` |
| [T-04](#t-04) | medium | OPEN | — | — | `.icp/cache/networks/local/state` | the local network state has no checkpoint height common to all five subnets, so it cannot be resumed |
| [T-15](#t-15) | medium | OPEN | — | — | `lib/utils.js` `formatTokenAmount` | the "canonical money layer" written this wave, documented at length, was imported by **nobody**. Seven copies of "divide by 1e8", not one |
| [T-32](#t-32) | medium | OPEN | — | — | `PokerTable.svelte:313` `max_players ?? 9` | **every table entry first paints a NINE-seat ring.** At a 6-max table on a phone the felt is 45.3%→50.7% of the frame for **336 ms** and then jumps to 60.6%; on desktop 33.2% for **304 ms** and then 31.7%. Wave 5's headline 60.6% is the settled state and was never distinguished from the first paint |
| [E-94](#e-94) | medium | FIXED | 13 | `./scripts/assert-read-only.sh --selftest`, which the guard runs on ITSELF before it judges any file, now carrying the four demonstrated bypasses as probe lines and requiring 5 of 5 mutating hits and 5 of 5 credential hits. Verified against hostile files: a workflow carrying `secrets.DEPLOY_KEY_PEM_BLOB` + `icp identity use`, and one carrying `icp canister create` + `icp canister migrate-id`, are now both REFUSED where both previously printed `ok … read-only, anonymous` and exited 0 | `scripts/assert-read-only.sh` `MUTATING` and `CREDENTIAL` | **the guard on the credential-free cycles monitor was a list of yesterday's incidents, not a rule.** `CREDENTIAL` matched `secrets\.IC_` only, so any secret not named `IC_*` — `secrets.DEPLOY_KEY_PEM_BLOB` — passed; `MUTATING` listed `install/stop/start/delete/top-up/snapshot/settings` and omitted `create`, `migrate-id`, `uninstall`, `update-settings` and every `icp identity` subcommand, so `icp identity use monitor` was `read-only, anonymous`. The exact historical regression (`secrets.IC_DEPLOY_IDENTITY`) was blocked and nothing near it was. Widened to the whole write half of the CLI surface and to every `secrets.` reference. **Still not transitive** — the guard reads the workflow, not the scripts a workflow invokes, so a `run: ./scripts/anything.sh` is unexamined; that is named here rather than fixed, because making it transitive means resolving arbitrary shell |
| [E-95](#e-95) | high | FIXED | 13 | reproduced and fixed locally with the exact command the job runs: `DFX_NETWORK=ic VITE_CANISTER_ID_LOBBY=aaaaa-aa VITE_CANISTER_ID_HISTORY=aaaaa-aa npm --workspace src/cleardeck_frontend run build` -> `ABORT: building for MAINNET, but the canister ids do not match the roles in .icp/data/mappings/ic.ids.json`; the replacement, `npm --workspace src/cleardeck_frontend run build:mainnet` with no ambient `DFX_NETWORK`, builds AND verifies (`8 of 8 checks passed`) | `.github/workflows/ci.yml` job `Build frontend` | **main's CI has been red continuously since 2026-08-07 for a reason that is a CI wiring fault, and a permanently red pipeline is the reason a fund-safety red would be invisible.** The job ran `npm run build` with `DFX_NETWORK: ic` and `VITE_CANISTER_ID_* = aaaaa-aa`, under a comment saying *"Placeholder IDs: the bundle resolves real IDs at deploy time from .env"*. That comment stopped being true when the mainnet-id cross-check landed in `vite.config.js`: an `ic` build whose ids disagree with the TRACKED `.icp/data/mappings/ic.ids.json` now aborts, and placeholders can never agree. So [H-23](#h-23) landed its fund-safety jobs onto a pipeline that was already failing, where **a new red is indistinguishable from the red that has been sitting there for two days**. Switched to `build:mainnet`, the one named path, which reads the ids from the tracked mapping itself, refuses a contradictory ambient `DFX_NETWORK` (so the step sets none) and verifies the bundle before exiting 0 |
| [E-96](#e-96) | low | OPEN | — | — | `SolvencyNotice.svelte` `visible`; `interpretSolvency` in `src/cleardeck_frontend/src/lib/solvency.js` | **money the table is holding for somebody it cannot name renders NOTHING on the deposit screen.** Found in wave 14 by SERVER-RENDERING the shipped `SolvencyNotice.svelte` for each reachable state rather than reading it. With 1 ICP sitting at the shared main account and nobody credited with it, the fixed canister answers `CanPayEveryone`, `difference_e8s = 0`, `unattributed_at_main = 100000000`, and NAMES the money in `summary` — and the notice hides itself, because `visible` is `state !== covered`. The verdict is right (the table holds exactly what it owes, so this is not a solvency warning) but the person whose ICP went to [FINDING 34](SECURITY-FINDINGS.md#finding-34)'s old shared address gets no signal on the screen they would look at. `interpretSolvency` does not even extract `unattributed_at_main`. NOT fixed in wave 14 on purpose: adding money figures to that screen requires a matching site in `tools/shots/lib/chain-agreement.mjs` — the token census forbids an unasserted money-shaped token — and that needs the shots harness against a live replica |
| [E-99](#e-99) | high | FIXED | 14 | `cd tests/money_safety && cargo test --test solvency_definition -- the_currency_guard_refuses_while_a_payout_is_still_in_flight` — verified RED by reverting `total_liability()` alone to its five-term sum on the same tree: `guard_liability=0`, `owed=0`, and `flip to BTC with a payout in flight -> ACCEPTED currency now BTC` | `total_liability` / `main_attributed_claims`, `src/table_canister/src/lib.rs`; read by `refuse_currency_change_while_funded` | **the currency guard read a liability of ZERO on a canister with an irreversible 5 ICP withdrawal still open in its own journal, and accepted the flip.** `total_liability()`'s own doc comment has argued since [FINDING 29](SECURITY-FINDINGS.md#finding-29) that *a table with unfinished ledger operations is a funded table* — re-issuing an open intent after a flip sends the retry to a chain where the original transaction does not exist, so the ledger's deduplication cannot fire and the movement happens twice. That argument was implemented for `pull` and `sweep` (`journalled_incoming_total()`) and **never for `payout`, which was in no term of the guard at all**; `main_uncredited_observed()` then SUBTRACTED the open payout, so a drained table with a withdrawal in flight summed to exactly zero. Closed as a side effect of the one-definition rewrite for [FINDING 43](SECURITY-FINDINGS.md#finding-43) / [FINDING 38](SECURITY-FINDINGS.md#finding-38), with no special case: `main_attributed_claims()` carries `open_payout_total()`, so `total_liability()` cannot be zero while any intent is open |
| [E-97](#e-97) | medium | OPEN | — | — | `tests/money_safety/tests/cycles_runway.rs` `print_runway_table` call sites | **the suite that produces the runway table prices an open browser tab as a heartbeat stream.** Its "500 hands/day with N tabs open" rows are built as `idle + net_per_hand*500 + hb_per_player_day*seats`, and `hb_per_player_day` is the 10-second heartbeat only. That is [E-92](#e-92)'s arithmetic, in code rather than in a comment: measured, an open tab costs 0.0572 T/day typical and 0.1929 T/day at the client's ceiling against the 0.0618 T/day the heartbeat contributes, so even the FIXED client makes those rows read low, and they read 18x low against the client this suite was written beside. The rows that involve no tabs (idle, N hands/day) are unaffected and agree with wave 14's independent control to within 7%. Not fixed in wave 14: it is another owner's suite and the honest correction is to add a measured per-tab term to the derivation and re-run it under PocketIC, not to edit the labels. Everything outside that suite now reads `tools/cycles/burn-table.json` |
| [E-100](#e-100) | high | FIXED | 14 | `./scripts/cycles-runway.sh --selftest` (also `./scripts/dev.sh cycles`) | `scripts/cycles-runway.sh` `parse_status` | **the cycles monitor never read the recent burn rate, and its own selftest could not tell.** `recent_burn_per_day` is `opt nat`, so the wire says `recent_burn_per_day = opt (2_604_344_185_140 : nat)`; the parser matched `field = <digits>` and therefore read it as ABSENT against every real canister, fell back to `or 0`, and the `max(lifetime, recent)` below it always chose the LIFETIME AVERAGE — the gauge the whole script was written to stop trusting, and the one that reads HIGH on a table that has just got busy. Found in wave 14 while checking the monitor against a live local canister that was reporting 2.60 T/day: `num('recent_burn_per_day')` returned `None`. **Every fixture in the selftest wrote the field as a BARE nat, a shape no module has ever emitted**, so five green checks proved the parser worked on a canister that does not exist. Replayed on the pre-fix parser with a wire-shaped fixture: it reports **225 days for a table burning 2.05 T/day**, which is 4. The JS reader was never affected — Candid decodes `opt` to an array and `cycleRunway.js` reads it through `optBig` |
| [E-98](#e-98) | high | FIXED | 14 | `./scripts/dev.sh test` step 4 → `cargo test --test deposit_floor` → `the_top_up_the_dust_refusal_names_actually_makes_the_balance_claimable` (it SENDS the figure the canister prints and requires the claim to succeed) and `sweepable_and_refundable_predict_what_the_two_doors_actually_do` (it asks the flag, then performs the action, at every boundary) | `deposit_custody_sentence`, `deposit_custody_of`, `src/table_canister/src/lib.rs` | **WAVE 12 MOVED THE SWEEP THRESHOLD AND LEFT EVERY SENTENCE ABOUT IT BEHIND.** The Rule-3 floor raised what `claim_external_deposit` will take from `> transfer_fee` to `>= min_external_deposit`, and the three player-facing statements built on the old threshold were not moved with it, so the canister spent two waves giving instructions it would refuse to carry out. (1) The dust refusal named a top-up of `fee + 1 - amount`: driven on the pre-fix build, *"It said to send 2 e8s more to the same address; that was done, the address now holds 10001, and the claim still refused"* — a second wasted ledger fee and a second refusal, for a player who was already stuck. (2) `deposit_custody_sentence`, the ONE place FINDING 28 centralised this prose precisely so the four surfaces could not diverge, told a player holding the advertised 20,000 to *"call claim_external_deposit(), which moves it and credits you 0.0001 ICP"*. (3) `sweepable` stayed `observed_amount > transfer_fee` under a doc comment reading *"true when claim_external_deposit() would sweep this amount right now"* — see [E-89](#e-89), where the drain quoted it back as evidence that stranded money was reachable. **Found by DRIVING the instruction instead of reading it**; every gate here does the thing the sentence says and checks the outcome, because a test that parses prose agrees with whatever the prose says |
| [E-93](#e-93) | medium | OPEN | — | — | `cargo` feature unification; `recipes/rust-reproducible.hbs` line 77 (one package) vs `scripts/check-candid.sh` line 142 (the whole workspace) | **`history_canister.wasm` is not a function of its source alone: it changes with the `-p` set of the `cargo build` that produced it.** Measured on identical source, same directory, same toolchain, clean each time: `cargo build -p history_canister --target wasm32-unknown-unknown --release` -> `ea701f77…62e15`; `cargo build -p table_canister -p history_canister …` -> `9f511048…3db2d`. `table_canister` is `3890a6d4…b29a0d` under BOTH, so it is not machine noise — it is cargo unifying dependency features across the packages named in one invocation. **Nothing in the tree currently builds that way**, which is why this is `low` and not high: the deploy recipe runs `cargo build --package {{package}}` one package at a time, `tests/money_safety/src/wasms.rs` builds each canister on its own, and the two-path byte-reproducibility check passes (working tree and a `cp -Rc` clone agree to the byte on both modules). But the whole point of the vendored recipe is that **a stranger can rebuild and compare a hash**, and a stranger who types `cargo build --release` for two canisters at once gets a module that matches nothing. The recipe is the only thing standing between that and a false "this canister is not the code" |
| [E-13](#e-13) | low | OPEN | — | — (`dev.sh known-defects` marker, red on purpose) | `poker_core::hand::detect_straight` | prefers the wheel over a better straight; unreachable today, a trap for the obvious optimisation |
| [E-83](#e-83) | low | OPEN | — | — | `buy_in`, `src/table_canister/src/lib.rs:4789` | `join_table(seat)` silently auto-buys-in at `min_buy_in`, and a subsequent `buy_in(seat, amount)` on **your own** seat answers *"Seat is taken"*. It is taken by the caller. Topping up needs `reload`, which the error does not name. First interaction at the table, and it lies about who is sitting there |
| [E-14](#e-14) | low | OPEN | — | — | `leave_table` | missing the `check_rate_limit()?` that `player_action` has |
| [E-15](#e-15) | low | OPEN | — | — | `poker_core::side_pots::level_pot` | the `partial_contributions` term is provably always zero: dead code on a fund path |
| [E-47](#e-47) | low | OPEN | — | — | `TableView` in `table_canister.did` | nothing distinguishes **caller-relative** fields (`can_check`, `call_amount`, `is_my_turn`, `min_bet`) from **global** ones (`action_on`, `current_bet`, `phase`), and no field says what the seat on action may legally do. The auditor read `can_check = true` while the seat on action owed 5,000,000 and got *"Cannot check, there's a bet to call"* eight times |
| [E-48](#e-48) | low | OPEN | — | — | `set_display_name` | reserved UI words are not rejected: `set_display_name(opt "You")` and `(opt "Dealer")` both return `Ok`. The auditor read a seat with `is_self = false` and `display_name = opt "You"` |
| [H-04](#h-04) | low | OPEN | — | — | `src/table_canister/src/lib.rs` seam | 6 of the 7 seam mutations die at canister level; the 7th (dropping the self-report line) has no reachable trigger in honest play. Re-run against the wave-3 payout rewrite: still **6 of 7**, same survivor |
| [H-06](#h-06) | low | OPEN | — | — | `src/table_canister/tests/money_safety.rs` | 2 of the 6 tests at the money-safety path do not test the canister |
| [H-13](#h-13) | low | OPEN | — | — | two `rng.rs` files | SplitMix64 is implemented twice, with different `below()` semantics |
| [H-47](#h-47) | low | FIXED | 12 | `dev.sh shots-selftest` and `npm run selftest` both call `tools/shots/selftest.mjs`, which DISCOVERS every `test-*.mjs` and REQUIRES the named ones to exist. 7 files, 7 run | the two lists of "the screenshot harness's own self-tests" have diverged. `npm run selftest` runs `test-solvency.mjs` and **not** `test-dock-overflow.mjs`; `dev.sh` runs `test-dock-overflow.mjs` and **not** `test-solvency.mjs`. So the repo's primary gate never runs the solvency-surface self-test, and the command the harness's own README gives never runs [E-63](#e-63)'s |
| [L-05](#l-05) | low | OPEN | — | — | lobby canister registry, `scripts/dev.sh up_wire` | [T-05](#t-05) determined: registering `btc_table_1` is **not** a frontend fix. The exact call, plus the 35 px it costs the phone layout |
| [T-03](#t-03) | low | OPEN | — | — | `src/lib/ic-config.js`, `vite.config.js` | the port is now build-time configurable (`VITE_LOCAL_GATEWAY_PORT`); `auth.js` and `oisy.js` still hardcode 4943 |
| [T-05](#t-05) | low | OPEN | — | — | every deploy path | `btc_table_1` is never registered in the lobby |
| [T-06](#t-06) | low | OPEN | — | — | `canister_ids.json` vs `.icp/data/mappings/ic.ids.json` | two mainnet id lists that can drift |
| [T-35](#t-35) | low | OPEN | — | — | local replica: no ckBTC ledger at `mxzaz-hqaaa-aaaar-qaada-cai` | `btc_table_1` is deployed locally but the ledger it needs is not, so **half the custody surface has no local test path**. Its mainnet twin is documented as holding real ckBTC |
| [E-02](#e-02) | fund-theft | FIXED | 2 | `dev.sh test` → `deposit_replay` (dr00–dr10) | `periodic_cleanup`, `deposit`, `notify_deposit`, `VERIFIED_DEPOSITS` | one real ledger transfer credited twice and the excess WITHDRAWN as real ICP; closed by a monotonic watermark plus a bounded record with one writer |
| [D-08](#d-08) | high | FIXED | 11 | CI `candid-check` → `./scripts/check-candid.sh --declarations` | `src/table_canister/table_canister.did` | the committed Candid did not describe the deployed code **and it is published on-chain** as `candid:service` metadata. Structural drift, not the 1,429-line raw diff. Same defect as [E-08](#e-08) / [FINDING 03](SECURITY-FINDINGS.md#finding-03), re-measured and closed with a gate |
| [D-09](#d-09) | high | FIXED | 11 | CI `candid-check` → `./scripts/check-candid.sh` (self-tests that it CAN go red before it judges) | `.github/workflows/ci.yml` `candid-check` | the "Candid interface drift" job ran on every PR, compared a raw byte diff so loud it had to be muted, and ended in `exit 0 # TODO`. It passed for the entire time [D-08](#d-08) was true |
| [D-10](#d-10) | high | FIXED | 11 | CI `deploy-ic.yml` pins `ic-wasm` by version | `.github/workflows/deploy-ic.yml` | the mainnet deploy installed an **unpinned** `ic-wasm`, in a step called "Install pinned toolchain", so the bytes shipped to canisters holding real funds depended on whatever upstream had released that day |
| [D-13](#d-13) | high | FIXED | 12 | `dev.sh hygiene` -> `custody disclosure present (FINDING 23)` (7 phrases, each negative-tested to go red on its own removal) + the `fully decentralized` claim-check; `dev.sh custody` produces the numbers it quotes | `README.md` line 7, `Security Considerations`, `Known Issues`; `DepositModal.svelte` | **the largest unbacked claim this project shipped.** The README said "fully decentralized" and "fair play without requiring trust" over canisters one key can empty, and disclosed only that a controller can destroy the HAND HISTORY — never the BALANCE, which is what an auditor then destroyed. The deposit screen said nothing at all. Now: a `Who can take your money` section with the measured transcript, an amber notice on the deposit modal itself, and both under a hygiene gate |
| [D-12](#d-12) | medium | FIXED | 11 | [`docs/DECLARED-VS-STORED.md`](DECLARED-VS-STORED.md) inventory, with a guard status per row | CI as a whole | nothing asked whether mainnet still matched `main`, so every claim about the running system decayed silently. Four defects share that shape and the sharpest — `btc_table_1` running at one tenth its declared stakes for its whole life — was found **by a player** |
| [E-01](#e-01) | critical | FIXED | 2 | `dev.sh test` → `regressions::reg01`; `settlement disagreements::pinned_e01` | `determine_winners` / `advance_to_next_street` | every showdown after post-flop betting paid only the pre-flop pot and destroyed the rest permanently; the payout basis is now rebuilt from the players' contributions at payout time |
| [E-07](#e-07) | critical | FIXED | 7 | `dev.sh test` → `admin_custody` (13 tests); `wave6_coherence::probe5` | `reset_table` + `admin_reinit_table` → `init_table_state` | one controller call destroyed 100% of a funded table's chips. Measured live on `table_3` before the fix: 40.00000000 ICP gone, canister still holding it, both players reading `get_balance = 0`. `reset_table` now REFUSES while the table holds custody; `admin_reinit_table` returns every chip to its owner's escrow first; `init_table_state` traps rather than rebuild over money |
| [E-16](#e-16) | critical | FIXED | 2 | CI `wasm32-tests`; `cargo test -p poker_core --test wasm32_golden` | `poker_core::shuffle` (was `lib.rs:2314`) | `(draw as usize) % (i+1)` truncated to 32 bits on wasm32, so NO third party could reproduce a deal from the revealed seed |
| [E-42](#e-42) | critical | FIXED | 5 | `dev.sh test` → `wave6_coherence::probe1`; `invariants::reachability` (M9) on every fuzz step | `plan_payouts` → `rank_claims` (NOT `record_hand_to_history`, which the first triage named); cause in `count_active_players` vs `live_claims` | **a funded table was locked with about 420 ICP unreachable through every path a player has.** One player stopped heartbeating pre-flop; every state-advancing call then trapped, and `withdraw`/`cash_out` refused because a hand was in progress. The cause was a hand ending as a fold-out with two live claims and no board; the trap was the messenger. [FINDING 15](SECURITY-FINDINGS.md) |
| [E-69](#e-69) | critical | FIXED | 8 | `dev.sh test` → `ledger_boundary` (8 tests, M14) + fault injection on one fuzz run in four | `deposit`, `claim_external_deposit`, `withdraw`, `notify_deposit`'s refusal, `get_custody_status`; the new "THE LEDGER-INTENT JOURNAL" section | **the only wave-7 blocker nobody could exercise, and the only unbounded one.** All three money doors moved real money on the ledger and settled the books afterwards, in the post-await continuation, with NOTHING written first. Measured on the module the auditor reviewed: 3.0 ICP pulled out of a player's wallet, 0 credited, and **eleven doors out of that state — player and controller — all closed**, including `notify_deposit` refusing the pull's real block index with *"it was credited to your balance when the pull happened"*. `withdraw()` has the same shape and the auditor did not name it: its debit and its pending flag are committed at the await, its refund only ever existed in the continuation, and an hour later the player was still locked out of the only door from escrow to the ledger. **Fixed:** an intent naming owner, amount and exact wire arguments is committed BEFORE every irreversible movement; retries are made safe by the LEDGER's own ICRC-1/ICRC-2 deduplication (`memo` + `created_at_time`), so a re-issue comes back `Duplicate{duplicate_of}` carrying the block index the lost continuation never saw; `resolve_my_ledger_intents()` is owner-drivable; the journal is `opt`-persisted and bounded by refusing to start rather than by forgetting. The literal trap was **not** forced — four mechanisms tried, each with its measured reason, in the finding. Gated by **M14 LEDGER/BOOKS COHERENCE** (`tests/money_safety/tests/ledger_boundary.rs`, 6 tests) plus fault injection on one fuzz run in four. [FINDING 29](SECURITY-FINDINGS.md#finding-29) |
| [T-33](#t-33) | critical | FIXED | 9 | CI `reproducible-build` + `docker-verification`; `scripts/verify-build.sh --two-paths`. The README's own box still says the opposite — [D-07](#d-07) | `Dockerfile`, `scripts/verify-build.sh`, `README.md` §Verify the Code, `icp.yaml` `shrink` | **nobody can check what code is running.** The deployed module hash matches no commit here; the build is not path-independent; the published Docker verification cannot compile (`COPY` omits `src/poker_core`); and the procedure the README gives a reader is controller-only. The auditor proved this is not paperwork: the deployed binary locked a funded table and the source in this repo settled the identical state correctly |
| [E-03](#e-03) | high | FIXED | 2 | `dev.sh test` → `cargo test --workspace payout_tests::e03_*`; settlement oracle | `calculate_side_pots` → `poker_core::side_pots` | `state.pot` overrode the players' actual contributions in both directions, minting in one and destroying in the other through an `f64` ratio; it can no longer move a chip |
| [E-04](#e-04) | high | FIXED | 2 | `dev.sh test` → `deposit_replay::dr01`/`dr02`; wave 11: `deposit_surface::money_at_the_shared_main_account_is_recoverable_by_its_sender_and_by_nobody_else` | `notify_deposit` | could never credit a deposit (two independent decode bugs) and the ICP sent was stranded forever. **Re-verified wave 11**: it credits its sender in full and refuses everybody else, and reverting BUG A or BUG B separately each turns a gate red |
| [E-05](#e-05) | high | FIXED | 2 | `dev.sh test` → `regressions::reg05`/`reg08`; `settlement disagreements::pinned_e05` | `leave_table`, `cash_out`, `check_timeouts` | a seat vacated mid-hand orphaned its stake, moving contested money into the deepest stack's exclusive pot; the stake is now recorded independently of seat occupancy |
| [E-06](#e-06) | high | FIXED | 5 | `dev.sh test` → `regressions::reg09`; `timers` | `check_timeouts` + `count_players_can_act` | on `table_1`/`btc_table_1` the disconnect and action timeouts were both 30 s, so one lull ran the whole board out and settled. Participation no longer reads `status`, and the two thresholds can no longer race: `reg09` now leaves the hand IN PROGRESS |
| [E-30](#e-30) | high | FIXED | 2 | `dev.sh test` → `cargo test --workspace coherence_regressions` | `player_action`, the `AllIn` arm | an all-in that raises by less than a full min-raise reopened the betting. The first fix implemented "closed if facing anything at all", which is not the rule and is a regression on CUMULATIVE short all-ins (TDA 47-A). Now `amount_owed < min_raise`; pinned by `coherence_regressions.rs` |
| [E-31](#e-31) | high | FIXED | 2 | `dev.sh test` → `cargo test --workspace coherence_regressions` | `player_action` timer check | an expired action timer was refused but never resolved, so the table wedged. Resolving it before the whose-turn check then let a message composed on the flop be APPLIED on the turn; such an action is now refused while the table still unwedges |
| [E-32](#e-32) | high | FIXED | 5 | `dev.sh test` → `regressions::reg09`; `cargo test --workspace hand_membership` | `is_betting_round_complete` + `count_active_players` + `count_players_can_act` + `find_next_active_seat` + `sit_out` | a `Disconnected` (or mid-hand `sit_out`) seat was skipped by the betting round yet stayed eligible for the pot: a free showdown for money already in, measured at **0.98 big blinds a hand**. Participation is now the SAME predicate as eligibility |
| [E-37](#e-37) | high | FIXED | 3 | `dev.sh test` → `invariants::principals` (M8); settlement PRINCIPAL column | `Stake`, `plan_payouts`, `apply_payouts` (`principal_of` deleted) | the E-05 fix paid a departed player's refunded stake to whoever took their chair. Composed end to end on the real canister: `-2000000` from the player who left, `+2000000` to the stranger in her chair, every total balancing. The owner now travels with the stake and `M8_PRINCIPAL_ATTRIBUTION` + a PRINCIPAL column in the settlement oracle gate it. [FINDING 13](SECURITY-FINDINGS.md) |
| [E-38](#e-38) | high | FIXED | 3 | `dev.sh test` → `invariants::upgrade_across_versions` (M7) | `PersistentState::deposit_watermark`, `TableState::departed_stakes` | two agents each added a non-`opt` persisted field. As shipped an upgrade was REJECTED; fixing only the first makes the upgrade SILENTLY destroy every chip at the table (re-measured on the wave-3 fixture: 594000000 seated e8s and three players' hole cards, gone, upgrade reported successful). Both are now `opt`; M7 upgrades `801aa79` state into the current wasm with every e8, stack, card and the anti-replay record intact, and `pre_upgrade` traps rather than proceeding after a failed save. [FINDING 14](SECURITY-FINDINGS.md) |
| [E-40](#e-40) | high | FIXED | 5 | `dev.sh test` → `wave6_coherence::probe1`; `invariants::reachability` (M9) on every fuzz step | `record_hand_to_history` (`lib.rs:871`) | the only unguarded `evaluate_hand` call left in the canister. Observed **trapping on the live local canister** during ordinary browser play: `IMPOSSIBLE HAND … got 0 community`. A trap here cannot settle the hand |
| [E-43](#e-43) | high | FIXED | 6 | `dev.sh test` → `cargo test --workspace hand_membership`; `wave6_coherence::probe4` | `count_players_can_act`, `count_active_players`, `is_betting_round_complete`, `check_timeouts` | a seated player who stops heartbeating is dropped from the betting round and **stays fully eligible for the pot**. Client-controlled, so it is an exploit: call the flop, stop heartbeating, get the turn and river free with full pot equity and immunity from any further bet |
| [E-45](#e-45) | high | FIXED | 7 | `dev.sh test` → `admin_custody::currency_cannot_be_changed_while_the_canister_owes_anybody_anything`; `coherence_w8::finding_33_an_open_sweep_keeps_the_currency_guard_shut_and_the_flip_is_reversible` | `reset_table`, `admin_reinit_table`, `admin_update_config` | `TableConfig::currency` selects the LEDGER `withdraw` pays from, and a controller could change it while balances existed, making every one of them unpayable without ever writing `BALANCES` |
| [E-49](#e-49) | high | FIXED | 5 | `dev.sh test` → `cargo test --workspace` → `history_canister::a_hand_reusing_a_number_after_a_table_reset_is_still_stored` | `history_canister` de-duplication key | the first idempotency key was `(table_id, hand_number)`. `reset_table` restarts hand numbering at zero, so after a reset four genuinely new hands were **silently discarded as duplicates** while the table was told `Ok` four times: `get_total_hands` moved 4 -> 5. Now keyed on the seed hash too |
| [E-52](#e-52) | high | FIXED | 7 | sweep: `toast-notices` (5/5 notices unoccluded under a real error toast, both viewports) | `.toast` vs `.alpha-warning-banner` in `src/cleardeck_frontend` | the app's own error toast was `position: fixed; top: 80px; z-index: 100` with no width or height bound. At 390x844 it covered the whole alpha-warning banner: the no-rake property **9 of 9 sample points covered**, the four protection notices 3 of 9. HARD RULE 2 says all four must be ON SCREEN at any viewport on any view. No screenshot scene raised a toast, so the repo's own occlusion gate had never seen it |
| [E-54](#e-54) | high | FIXED | 7 | `dev.sh test` → `timers::the_table_settles_itself_with_no_external_caller` | `src/table_canister/src/lib.rs` "THE ON-CHAIN CLOCK"; `tests/money_safety/tests/timers.rs` | **nothing on chain moved the game.** `ic-cdk-timers` was declared and `set_timer` appeared nowhere in `src/`. Measured before: a live pre-flop hand holding 3,000,000 e8s sat unchanged for **20 simulated minutes** with no client attached, both seats still `Active` — the dead window was not 5.5 minutes, it was **unbounded**. After: the hand resolves itself in **30 s** and every seat is released with chips back in escrow at **210 s**, with zero ingress messages. [FINDING 19](SECURITY-FINDINGS.md#finding-19) |
| [E-56](#e-56) | high | FIXED | 7 | `dev.sh test` → `timers::a_hand_stalled_past_its_grace_is_played_out_not_voided` | the on-chain clock's stuck-hand escalation | the new clock **VOIDED A PLAYABLE HAND**: it refunded every stake in a hand whose clock was merely *stale*, without ever trying the ordinary timeout path. Reached by any stall in which the canister does not execute — including **a canister frozen for want of cycles and then topped up**, so it composed directly with [E-55](#e-55). The grace is now measured from when the canister first SAW the clock overdue. Reverting the fix leaves the fuzzer GREEN, so it has its own gate |
| [E-57](#e-57) | high | FIXED | 7 | `dev.sh test` → `invariants::custody` (M10) | `cash_out`, `leave_table`, `get_custody_status`, `TableView`, `withdraw`'s refusals | a player could walk away from a table with money in the pot and **every surface told them they had nothing**. A hand no message can move is now settled before the seat is vacated, a live hand cannot outlive its last player, and four surfaces state the committed stake and name `abandon_stuck_hand`. [FINDING 18](SECURITY-FINDINGS.md#finding-18) |
| [E-59](#e-59) | high | FIXED | 7 | `stall_agreement` (M13) — **run by `dev.sh test` step [5/9] and the `deep` CI tier since wave 14** ([H-45](#h-45) closed) | `hand_is_stuck` vs `clock_should_abandon`; `cash_out`, `leave_table`, `get_custody_status`, `TableView.hand_is_unmovable` | **the fourth cross-agent defect.** After a stall the canister holds two beliefs about one hand: the clock says "playable" and every player-facing surface says "dead". Measured on one state: the clock alone pays `alice=+0 bob=+4000000`; `abandon_stuck_hand(alice)` pays `alice=+2000000 bob=+2000000`. Totals conserve in both. `cash_out` is worse than the recovery method — it takes the whole stack out of a live hand too — and `get_custody_status()` **tells the losing player to press the button**. No gate in the tree could reach it, because `World::advance()` always lets a round run. **Fixed:** `clock_should_abandon` deleted, one predicate for all six surfaces, and it is about ATTEMPTS — the canister must have watched the resolution path fail across 3 committed messages and 300 s. Gated by **M13 ONE BELIEF** (`tests/money_safety/tests/stall_agreement.rs`): 138 forked states, 118 rows paying different recipients → 0, 20 `Err`-replies that changed state → 0. [FINDING 25](SECURITY-FINDINGS.md#finding-25) |
| [E-61](#e-61) | high | FIXED | 7 | `dev.sh test` → `shots-selftest test-rake.mjs`; sweep: 0 `RAKE TAKEN` on 24 shots | `tools/shots/lib/chain-agreement.mjs` (`foldArchivedHand`) | **the only gate that asserts the no-rake property against the archive is permanently red for a reason unrelated to rake.** It reads `Number(r.rake)` off `get_hands_by_table`, which returns `vec HandSummary`, and `HandSummary` has no `rake` field. `NaN !== 0`, so every archived hand reports `RAKE TAKEN: rake=NaN`. The comment thirty lines above says absence of a field *"is now a structural failure, never a silent NaN"* — that fix was applied to one field and not to the adjacent one |
| [E-62](#e-62) | high | FIXED | 7 | `dev.sh test` → `deposit_floor` (6 tests) + `const _: () = assert!` | `ICP_MIN_DEPOSIT_AMOUNT` / `ICP_MIN_WITHDRAWAL_AMOUNT`, `withdraw`, both money modals | **the canister accepted money at the minimum it advertises and would not give it back.** `deposit()` took 20,000 e8s; `withdraw()` refused anything under 100,000; `buy_in` refused it too; `get_custody_status` called it a healthy balance. One number per currency now, the relation `min_withdrawal <= min_deposit` is a `const _: () = assert!` so the old value is a BUILD failure, and a caller's whole remaining balance can always leave at any size the ledger can move. [FINDING 27](SECURITY-FINDINGS.md#finding-27) |
| [E-63](#e-63) | high | FIXED | 8 | `dev.sh test` → `shots-selftest test-dock-overflow.mjs`; sweep: `table-allin`/mobile 81 pixel pairs, 0 occluded | `.action-dock` in `PokerTable.svelte`; `tools/shots/verdict-gate.mjs` | **the pixel gate caught a regression and nobody acted for a whole wave.** 8.7% of `"0.20 ICP"` painted over by `div.stage` at 390x844: a fixed-height dock with an 87 px wallet panel in a 36 px row, spilling under a positioned sibling. Fixed by sizing the dock to its contents — the first attempt bought the occlusion green with a **felt red** (50.7% → 44.1% against a 45% floor) and the new fixture caught that too. The walked-past half is fixed by making the last recorded verdict a gate |
| [E-66](#e-66) | high | FIXED | 8 | `dev.sh test` → `invariants::archive` (M12); `record::check_archived_participants` each fuzz step | `docs/SHUFFLE-SPEC.md` §4; the archive's participant list | the permanent hand record named the wrong people — built from the seats **as they stood at settlement**, so short by one on any hand somebody left and long by one on any hand somebody joined — and the shuffle specification told verifiers to count from it. [FINDING 30](SECURITY-FINDINGS.md#finding-30) |
| [E-70](#e-70) | high | FIXED | 10 | `dev.sh test` → `fuzz` (`invariants::solvency` each step) **and** `solvency.rs` (12 tests), which `dev.sh test` step [5/9] and the `fast` CI tier have run since wave 14 ([H-45](#h-45) closed; 12 passed in the wave-14 run) | `MAIN_CUSTODY`, `refresh_solvency`, `get_solvency`, `get_custody_status().canister_solvency` | **THE FIFTH CROSS-AGENT DEFECT.** Every observation instrument was built for the deposit SUBACCOUNTS and none for the MAIN account, so `admin_audit_deposit_custody` replied `(1 audited, 0 held, 0 unaudited)` on a canister holding 5 ICP and the currency guard accepted a flip that closed the money's only recovery door. Nothing was added that can edit a balance. [FINDING 35](SECURITY-FINDINGS.md#finding-35) |
| [E-72](#e-72) | high | FIXED | 10 | `solvency::components_sum_to_total` — **run by `dev.sh test` step [5/9] and the `fast` CI tier since wave 14** ([H-45](#h-45) closed) | `tests/money_safety/src/table_api.rs` `CustodyStatus` | the harness's `CustodyStatus` mirror silently dropped `unfinished_ledger_ops` — the field that carries [FINDING 29](SECURITY-FINDINGS.md#finding-29)'s money — under a comment saying it is *"Mirrored in FULL on purpose"*. Candid record subtyping drops an undeclared field without a warning. [FINDING 36](SECURITY-FINDINGS.md#finding-36) |
| [E-73](#e-73) | high | FIXED | 10 | `dev.sh test` → `fuzz` (`invariants::solvency::check_solvency_report_is_coherent` each step) | `total_liability()`; `get_solvency().guard_liability` | the last custody guard's **only** input had one caller, no query, no surface and no gate, so the only way to sample it was to attempt the destructive operation it guards. [FINDING 37](SECURITY-FINDINGS.md#finding-37) |
| [E-78](#e-78) | high | FIXED | 11 | `dev.sh test` → `oldest_cluster` → `finding22_the_recovery_door_refuses_a_hand_that_can_still_be_played` + `finding22_a_hand_a_controller_does_end_is_closed_and_permanently_marked`; `dev.sh test` → `admin_custody::admin_reinit_table_mid_hand_returns_the_pot_to_the_players_who_put_it_in` + `admin_custody::a_controller_ending_a_hand_pays_a_vacated_seats_stake_to_its_owner_not_its_new_occupant` (wave-11 reconciliation: a seat that changed hands mid-hand, asserted PER PRINCIPAL) | `return_all_table_custody_to_escrow` via `admin_return_all_chips_to_escrow` / `admin_reinit_table` | **[FINDING 22](SECURITY-FINDINGS.md#finding-22).** The recovery door worked on a hand being actively played, so a controller who had read every hole card could decide whether the hand happened — conserving to the e8, so no invariant could see it — and it left the hand OPEN with cards on the board and every stack at zero. Now refuses unless nothing can move the hand, closes it through the permissionless `settle_unmovable_hand`, and marks every credit `pot_type="refund:ended-by-controller"` |
| [H-49](#h-49) | high | FIXED | 11 | `dev.sh test` step 5 / `dev.sh settlement` → `cargo test --test settlement` → `a_seat_folded_by_its_own_clock_and_cashed_out_mid_hand_settles_by_the_rules`; `suite::timed_out_seat_cashes_out_mid_hand` in `run_all` | `tests/settlement/src/suite.rs`, `src/drive.rs` | the settlement oracle — the ONLY instrument that asks who was PAID rather than whether the totals balance — had never executed `cash_out` or `check_timeouts` in 53 compared hands, because every vacating scenario was written as *"`leave_table`, and `cash_out` if that fails"* and `leave_table` never fails. That is [FINDING 08](SECURITY-FINDINGS.md#finding-08)'s door, the one reachable by a plain disconnect |
| [H-01](#h-01) | high | FIXED | 2 | `dev.sh test` → the harness always builds and verifies the installed module hash | `tests/money_safety/src/wasms.rs` | the stale-artifact path is gone: the harness always builds, prints the sha256 as the first line of every run, and verifies the installed module hash |
| [H-02](#h-02) | high | FIXED | 2 | `dev.sh test` → `invariants::classifier` | `tests/money_safety/src/invariants.rs` | `CRITICAL:` and any unenumerated `BUG:` line is now `SelfReportedFailure`, which nothing can excuse |
| [H-03](#h-03) | high | FIXED | 2 | `dev.sh test` → `invariants::classifier`; `documented::REGISTER` (now empty) | `tests/money_safety/src/documented.rs` | tolerance is now a NAMED register keyed on (invariant, check, direction, magnitude); a rake is caught by `awarded_equals_payout_basis` |
| [H-16](#h-16) | high | FIXED | 3 | `dev.sh test` → `invariants::upgrade_across_versions` (M7) | `tests/money_safety/src/world.rs`, `wasms.rs` | `World::upgrade` reused `self.table_wasm`, so every "survives an upgrade" assertion was new-wasm-to-itself. `upgrade_to_module_under_test` + `previous_release_table_canister()` + M7 now walk `801aa79` state into the current wasm |
| [H-17](#h-17) | high | FIXED | 3 | `dev.sh test` names `deposit_replay` explicitly | `scripts/dev.sh` `cmd_test` | `tests/deposit_replay.rs` -- the E-02 fund-theft reproducer and its ten regressions -- was named by no make target for the whole wave |
| [H-18](#h-18) | high | FIXED | 5 | `dev.sh test` → `deposit_replay::dr02` (retries the index after the rate-limit refusal) | `deposit_replay.rs` `dr02` | the named regression on the fund-theft primitive never presents the deposit block: the rate limiter skips it and the loop does not retry |
| [H-24](#h-24) | high | FIXED | 5 | sweep: `shuffleproof` (verdict reached, 52 deck slots, computed hash equals the canister's commitment) | `tools/shots/scenarios/shuffleproof.mjs` | the fairness scene asserts `.proof-item >= 2`, which is true **before** the verification runs. Both shipped shuffleproof PNGs show rungs 3 and 4 grey and "Re-deriving your cards locally", filed as verified |
| [H-28](#h-28) | high | FIXED | 3 | `dev.sh test` runs `fuzz` under `env -u`; `make fuzz-default` | `documented::TOLERATED_SELF_REPORTS` vs `documented::REGISTER` | the two halves of the tolerance mechanism did not meet: a tolerated `WARNING:` line became an `M1b BreakdownDrift` violation on a check no `REGISTER` entry named, so it BLOCKED. `cargo test --test fuzz` with no environment was RED at `fe72d46` (seed `0xc1ea2dec0003`, 220 ops shrunk to 12, 221 s). Not an engine defect: the 12 ops are E-36 exactly. Closed by the register entry, NOT by silencing the detector, and the invocation is now in `make test` and `make fuzz-default` |
| [H-32](#h-32) | high | FIXED | 4 | sweep: `handhistory` action log | `src/declarations/table_1/table_1.did.js` | the client's Candid declaration of `ActionRecord` omitted `phase` and `amount`, so the agent decoded three fields off a five-field record. **No client could show a call's or an all-in's amount, or the street any action happened on, however it was written.** Root cause is [E-08](#e-08) (the hand-maintained `.did`), which is a different owner's file and stays open |
| [H-36](#h-36) | high | FIXED | 9 | sweep: protected-notices 5/5 on all 24 shots, behind both money modals and the Verify Code dialog | every full-screen dialog vs `.alpha-warning-banner` | wave 4's *"4 of 4 phrases on screen … **and behind the open Deposit modal**"* is a GEOMETRY measurement. Hit-tested, a dialog's 72%-black scrim hides all four: with the hand-history modal open and its own copy of the notices suppressed, **0 of 5** protected phrases are on screen while **4 carriers sit in the DOM** |
| [H-37](#h-37) | high | FIXED | 5 | `dev.sh test` → `shots-selftest test-occlusion.mjs` | `tools/shots/lib/occlusion.mjs`, `lib/png.mjs`, `test-occlusion.mjs` | every gate in this repo read `textContent`, so a correct number with a card painted over it was photographed and filed as VERIFIED twice ([T-22](#t-22), [T-23](#t-23)). There is now a gate that judges PIXELS: effective paint order per CSS 2.1 Appendix E, hit testing, and a four-shot pixel differential per (figure, occluder) that decides. Across a 22-shot sweep a naive z-index gate would have raised **3,836** false flags and this one raises **0** |
| [H-40](#h-40) | high | FIXED | 5 | sweep: protected-notices | `tools/shots/run.mjs`, new `tools/shots/lib/felt-area.mjs` | wave 5 built a pixel-level notice gate and never wired it in: `protected-notices.mjs` was imported by **two** scenarios and by nothing else. Flipping one declaration, `.banner-strip { display: block }` → `display: none`: re-commits wave 4's exact crime with **every gate in the repo green**. Both halves now run centrally, for every scene at every viewport |
| [L-01](#l-01) | high | FIXED | 5 | sweep: `lobby` at four width ranges | `Lobby.svelte` `.list-pane` / `.tables-list` | every row's `Sit` / `View` / `Watch` control was **clipped** — 10.5 px of it, arrow included — at 1440×900, and at three more width ranges besides. A control a player clicks, cut off by `overflow: hidden`, at the project's own reference viewport |
| [T-01](#t-01) | high | FIXED | 9 | `npm run build:mainnet` (13 static + 23 rendered checks); its mutation self-test `build/verify-bundle.selftest.mjs` is **NOT RUN**, see [H-46](#h-46) | `src/cleardeck_frontend` build, root `.env` | the build now refuses to run without an explicit target network, loads the repo-root `.env` only for `-e ic`, and aborts if a local build resolves a mainnet id |
| [T-08](#t-08) | high | FIXED | 3 | sweep: chain-agreement on every table scene | `PokerTable.svelte` pot header | the table's headline POT was displayed at **2×** during every betting round, and disagreed with the pot-odds strip on the same screen. The headline is now `get_pot()` unmodified, the two legs shown beside it are a decomposition that sums back to it, and the pot-odds strip spells out the same figure. Gated by the screenshot harness on every table scene |
| [T-10](#t-10) | high | FIXED | 3 | sweep: page-health (0 console errors on every scene) | `PokerTable.svelte` `$effect` | `JSON.stringify` on a Candid `nat64` threw **19 uncaught TypeErrors in one ordinary hand**, killing the action log and starving the effects the fairness panel and hand history run on |
| [T-11](#t-11) | high | FIXED | 3 | sweep: chain-agreement on the table header | `+page.svelte` `.current-table-name` | the largest string on every table screen quoted the LOBBY's stale blinds: `6-Max - 0.01/0.02` on a table charging 0.05/0.10 and `9-Max - 0.01/0.02` on one charging 0.10/0.20. 18 scenes were filed "agrees with chain: yes" around it |
| [T-16](#t-16) | high | FIXED | 4 | sweep: felt-area at 390x844 | the whole client at 390×844 | the mobile playing surface was **19.7–21.1% of the screen against PokerNow's 52.1% on the identical device**, and two of the six mobile captures did not contain a poker table at all. Now **42.9–52.1%**, scroll-anchored, nothing off-frame |
| [T-18](#t-18) | high | FIXED | 4 | `dev.sh test` → `ui_limits` | `WithdrawModal.svelte:74` | the withdrawal confirmation printed the ledger **block index** as an ICP amount, and never stated the fee. Measured: withdrawing 1 ICP at block 1130 said "0.0000 ICP sent to your wallet" |
| [T-20](#t-20) | high | FIXED | 4 | sweep: protected-notices (5/5 on all 24 shots) | `src/index.scss` portrait rule | on a phone, on the table view and behind the open Deposit modal, **0 of the 4 protected notices were on screen** — 4 of 4 in the DOM, 248 px of scroll away. `make hygiene` greps the source and cannot see it. The de-duplication is kept; the copy that survives is now the TOP banner, verified 4 of 4 on screen |
| [T-21](#t-21) | high | FIXED | 4 | sweep: `handreplay` claim assertions | `HandHistory.svelte` replayer banner | the hand replayer asserted *"These cards were fixed before the hand was played … before any card was dealt"* under a green tick, in the same wave and product as `ShuffleProof.svelte`'s *"Not proven: that the commitment came before the cards"* |
| [T-22](#t-22) | high | FIXED | 5 | `dev.sh test` → `shots-selftest test-occlusion.mjs`; sweep | `.equity-badge` vs `.player-cards` in portrait | on a phone the winner's `100.00%` rendered as **`0%`** — the hero's own card covered the rest. Fixed and **measured on the rendered page in both directions** by the new pixel gate ([H-37](#h-37)): 60.4% of the badge's ink covered before, 0.0% after, on the same scenes against the same canisters |
| [T-25](#t-25) | high | FIXED | 5 | sweep: felt-area with all five notices on screen | the portrait table as a whole | with the notices on screen — the only shippable configuration — the wave-4 portrait redesign measures **18.3% / 16.4%** of the phone against **20.9%** for the geometry it replaced. The 42.9–52.1% headline exists only in the configuration that hid the notices. **Fixed by taking the vertical budget off the chrome, not the notices**: with all five protected phrases on screen and hit-tested, the portrait felt is now **60.6% (6-max) / 50.7% (9-max)**. The aspect was never changed |
| [T-31](#t-31) | high | FIXED | 4 | sweep: protected-notices behind both money modals | `.modal-backdrop` in `WithdrawModal.svelte` / `DepositModal.svelte` vs the notice banner | with **either money modal open, 0 of the 4 protected notices are unobstructed**, at 1440x900 AND at 390x844. `elementFromPoint` at the centre of each returns `.modal-backdrop` — `rgba(0,0,0,0.7)` + `backdrop-filter: blur(4px)`, z-index 200. Identical on `fe72d46`, so the wave-4 T-20 fix left this case open. All four are now restated INSIDE both dialogs: **4 of 4 unobstructed** on both viewports with either modal open |
| [T-36](#t-36) | high | FIXED | 5 | sweep: protected-notices | `+page.svelte`, `DepositModal.svelte`, `WithdrawModal.svelte`, `HowItWorks.svelte` | **HARD RULE 2 was live-broken across the whole desktop app.** The canonical sentence *"No rake is taken from any pot on any table"* was on screen on **4 of the 15 (surface, viewport) pairs** a player can reach, and on **1 of the 7 desktop ones**. Desktop lobby, desktop table, desktop table behind Deposit, desktop table behind Verify Fair: **4 of 5**. Portrait with FULL TERMS open, portrait behind Deposit: **4 of 5** |
| [T-38](#t-38) | high | FIXED | 9 | `npm run verify:deployed` | `routes/+page.svelte` "Deployed Canister Hashes"; `build/verify-deployed.mjs` | the three module hashes the app showed a player were three upgrades stale and **nothing in the repository compared them to anything**. `verify-deployed.mjs` now reads the live hashes and the app is generated from that |
| [T-40](#t-40) | high | FIXED | 9 | sweep: protected-notices with the Verify Code dialog open | `.modal-backdrop` in the Verify Code dialog vs `.banner-warning` / `.disclaimer-warning` | **0 of 5 protected notices legible with the Verify Code dialog open, at both viewports** — HARD RULE 2, in the one dialog whose subject is whether this deployment can be trusted. Measured on rendered pixels with `elementFromPoint`, not read |
| [D-04](#d-04) | medium | FIXED | 4 | `docs/DESIGN-BAR.md` §9.4.1 | `docs/DESIGN-BAR.md` §9.4.1 | "There is no real mobile lobby capture in the corpus" is false. `pokerstars/web-ps-gipsy-2.png` is one, indexed `real_gameplay=true`; the doc missed it by querying `scene == 'lobby-mobile'` when it is filed `mobile-portrait` |
| [E-09](#e-09) | medium | FIXED | 2 | `dev.sh test` → `tools/differential fast_subset` | `poker_core::{evaluate_hand, evaluate_five_cards}` | no input validation: duplicate cards, wrong card counts and short boards produce plausible impossible hands instead of trapping |
| [E-11](#e-11) | medium | FIXED | 8 | `dev.sh test` → `ledger_boundary::m14_withdraw_continuation_discarded_does_not_lock_the_player_out` + `::m14_journal_survives_an_upgrade` | `withdraw` across an upgrade | an upgrade mid-withdraw drops the reply callback, so the refund branch can never run |
| [T-41](#t-41) | critical | FIXED | 11 | the deploy workflow now runs `npm run build:mainnet`, which refuses an unstated target and VERIFIES the bundle (13 checks) before exiting 0 — reproduced locally with the workflow's exact env, 13/13 passed | `.github/workflows/deploy-ic.yml` "Build frontend" | **THE MAINNET DEPLOY COULD NOT COMPLETE, and this wave made a redeploy mandatory.** The step ran a bare `npm --workspace src/cleardeck_frontend run build` with no `DFX_NETWORK`, which [T-01](#t-01)'s own wave-9 fix aborts (*"ABORT: the ClearDeck frontend build does not know which network it is for"*). It precedes the snapshot and the backend deploy, so **nothing** was deployed. Invisible because `ci.yml`'s frontend job sets `DFX_NETWORK: ic` — a gate proving the build works in an environment the deploy does not use. Two agents' work colliding: T-01 added the guard in wave 9, [FINDING 03](SECURITY-FINDINGS.md#finding-03) moved every module hash in wave 11 |
| [E-79](#e-79) | medium | FIXED | 11 | `dev.sh test` → `oldest_cluster` → `finding22_a_hand_a_controller_does_end_is_closed_and_permanently_marked` (the recorded credits must sum to what the hand collected) | `settle_unmovable_hand` vs `settle_hand` | the local 100-hand ring write lived inline in `settle_hand`, which `settle_unmovable_hand` does not call, so **every** hand refunded rather than won — `abandon_stuck_hand`, the clock, an exit door, the recovery door — read back from `get_hand_history` as `winners: [], community_cards: []` while the archive canister held every credit. Now one `record_local_hand_result` with two callers |
| [E-35](#e-35) | medium | FIXED | 2 | `dev.sh test` → `settlement disagreements::pinned_odd_chips` | `determine_winners` odd-chip rule | a chopped pot gave its WHOLE remainder to one seat; with three or more winners the rules give one chip each, clockwise from the button. Found by the settlement oracle (D-04) |
| [E-36](#e-36) | medium | FIXED | 6 | `dev.sh test` → `wave6_coherence::probe4`; `cargo test --workspace hand_membership` | `join_table` + `sit_in` + `is_in_hand` + `count_active_players` | a player who takes an empty chair MID-HAND and calls `sit_in()` is given the action and can bet into a hand they hold no cards in. **In wave 6 it grew a third-party victim:** that cardless seat is counted by `count_active_players`, so when the last card-holder is folded by its own clock the engine settles a "fold-out" with no live claim at all and refunds every stake -- **the hand is un-played and the fold-out WINNER loses the pot they won.** Measured on `5e07edf6` and re-measured on `306caef4`: a 52,000,000 e8 pot, three seats, all three back on exactly their buy-in. **Closed by making participation and eligibility ONE function** (`live_claims` calls `is_in_hand`; `is_in_hand` requires cards), which also stops a cardless seat being offered the action. Register entry and tolerated log line deleted with it. See [FINDING 17](SECURITY-FINDINGS.md#finding-17) |
| [E-39](#e-39) | medium | FIXED | 3 | `dev.sh test` → `fuzz` at its own defaults | `leave_table` | reduced `state.pot` via `return_uncalled_bet` without the paired `refresh_side_pots`, so the side pots a player is SHOWN stopped summing to the pot. Found by `make fuzz` at its DEFAULT 9 seeds; the wave that caused it ran 3 |
| [E-46](#e-46) | medium | FIXED | 10 | `dev.sh test` → `fuzz` (`invariants::solvency` each step) **and** `solvency.rs`, run by `dev.sh test` step [5/9] and the `fast` CI tier since wave 14 ([H-45](#h-45) closed) | `icrc1_balance_of` vs `admin_get_all_balances` + `admin_get_table_chips` + `get_pot` | no endpoint reconciles **funds held** against **liabilities recorded**. `table_2` holds 734,105,000,000 e8s against ~42,000,000,000 of recorded liabilities. Nothing is under-collateralised, but *"held by the canister and attributed to nobody"* is exactly what a lost deposit looks like and there is no view that tells the two apart |
| [E-53](#e-53) | medium | FIXED | 7 | `dev.sh shots` resolves the controller from the canister | `tools/shots/lib/ids.mjs`, `lib/frontend-build.mjs`, `run.mjs` | the harness resolved the controller from a four-name allowlist. On this machine the tables are controlled by `cyclepay-hotwallet` and the frontend asset canister by `oms-port-trial`, so **the whole screenshot sweep aborted** — and with it the pixel gate, the occlusion gate and the protected-notice gate — while an aborted run had already deleted the previous run's evidence |
| [E-60](#e-60) | medium | FIXED | 7 | `make hygiene` (512 KiB cap on tracked artifacts) | `artifacts/screens/**/manifest.json`, `.gitignore`, `scripts/dev.sh` | the screenshot manifest went from **763,703 to 7,612,380 bytes** in one wave and `make hygiene` exited 1. The manifest is build output and is now gitignored; the tracked evidence is `INDEX.md` plus a new **10,767-byte** `verdicts.json` carrying every scene's verdict and every failure headline. ~23 MB of machine-generated JSON leaves the index |
| [E-64](#e-64) | medium | FIXED | 8 | sweep: token census `0 unaccounted for` on every table scene, both viewports | `dom-scrape.mjs`, `chain-agreement.mjs`, `token-census.mjs` | the FINDING 18 committed-stake readout rendered real ICP that **no gate tied to any canister figure**, on five scenes. Now asserted against `get_table_view().my_committed_in_pot` and `hand_number` — and its ABSENCE while the canister says money is committed is a structural failure, because FINDING 18 coming back must not look like "this scene has no committed block" |
| [H-05](#h-05) | medium | FIXED | 2 | `dev.sh test` → `tools/differential fast_subset` | `tools/differential/src/checks/reference_probe.rs` | both references are now called per probe and the measured verdict is reported; every probe emits a finding only while the engine still accepts the input |
| [H-07](#h-07) | medium | FIXED | 1 | `dev.sh shots` (only a verified scene gets the canonical filename) | `tools/shots/run.mjs:119` | an unverified scene wrote the canonical PNG filename |
| [H-09](#h-09) | medium | FIXED | 2 | sweep: `lobby` asserts `data-lobby-state` | `+page.svelte` `<main>`, `tools/shots` | spinner and lobby are now a real either/or; `data-lobby-state` is asserted by the lobby scene |
| [H-12](#h-12) | medium | FIXED | 4 | `dev.sh test` → settlement PRINCIPAL column; `invariants::principals` | all harnesses | nothing proved the RIGHT player won. `tests/settlement` is the independent oracle; since wave 4 it also asks WHO, by principal, on every hand it runs, and the money-safety harness asks the same question on every hand its fuzzer completes. [Matrix](#attribution-what-was-planted-and-what-convicted-it) |
| [H-14](#h-14) | medium | FIXED | 5 | `docs/DESIGN-BAR.md` §1.1/§1.2/§10; sweep: `felt-area.mjs` | `docs/DESIGN-BAR.md` §1, bars 1/2/7/10/15 | the felt geometry is mis-measured, so three of four bars would reject correct work |
| [H-19](#h-19) | medium | FIXED | 5 | `dev.sh test` → `invariants::classifier::register_entries_are_all_still_needed` | `tests/money_safety/src/documented.rs` | `register_entries_are_all_still_needed`, documented as the thing that stops a stale tolerance surviving a fix, does not exist |
| [H-20](#h-20) | medium | FIXED | 3 | `dev.sh test` → `invariants::classifier` | `invariants/relational.rs` + `documented.rs` | the payout fix moved a live defect's self-report from `CRITICAL:` to `WARNING:`, which the detector does not match; 296 of them went unreported in one fuzz run |
| [H-31](#h-31) | medium | FIXED | 4 | sweep: `handreplay` | `tools/shots/scenarios/handreplay.mjs` (new) | the replayer is now opened, walked street by street and asserted at both viewports: every board card against `community_cards`, every log line against the hand record, the fairness claims, and the four protected notices hit-tested on the rendered page |
| [H-33](#h-33) | medium | FIXED | 4 | sweep: `handhistory` staging assertion | `tools/shots/scenarios/handhistory.mjs` staging wait | `querySelectorAll('.community-cards .card').length >= 5` was **true the instant the board frame mounted**: `PokerTable.svelte` always renders `Array(5)` of `<Card>` and an undealt slot is still a `.card`. A wait that cannot fail stood in for the one piece of state the scene depends on |
| [H-34](#h-34) | medium | FIXED | 5 | `dev.sh test` → `shots-selftest test-occlusion.mjs` | `tools/shots/lib/occlusion.mjs` | the new pixel gate had no notion of a dialog, so **every modal scene failed it**, the pre-existing `handhistory` scene included: a dialog covering the table behind it is what a dialog is for. The gate now identifies the overlay LAYER (fixed ancestor covering ≥40% of the viewport, or `role="dialog"`/`aria-modal`) and reports a page figure covered by an overlay as `behind-an-overlay` instead of failing; a figure INSIDE an overlay is still gated, so a dialog covering its own numbers still fails. `handhistory` and `handreplay` are verified at both viewports with 0 occluded and no `SHOTS_OCCLUSION=report` |
| [H-35](#h-35) | medium | FIXED | 4 | sweep: `handreplay` claim assertions | `HandHistory.svelte` proof panel, download file, list foot | three claims [T-21](#t-21) left behind in the same modal: the commitment row was labelled *"committed before the deal"*, the downloadable audit file called the field `commitment_published_before_deal`, and the list foot said *"**Every** hand above was re-derived in this browser"* while listing hands whose seed is still sealed |
| [H-38](#h-38) | medium | FIXED | 7 | `dev.sh test` → `cmd_shots_selftest`. The two selftest lists have since diverged — [H-47](#h-47) | `scripts/dev.sh`, `Makefile`, `.github/workflows/ci.yml` | the harness's three self-checks — the token census, the money parser and the pixel gate — are named by **no make target and no CI job**, the same shape as [H-17](#h-17). `npm run selftest` in `tools/shots` runs all three in ~15 s with no replica; wiring it into `cmd_test` is one line in a file this task does not own |
| [H-39](#h-39) | medium | FIXED | 7 | `make hygiene` | `scripts/dev.sh` `cmd_hygiene`, size rule | `make hygiene`'s payload check counts **modified tracked files** as untracked payload (1,056 KiB of them right now), so a large wave plus one screenshot run makes it red for a reason that has nothing to do with large or binary files. Two wave-5 agents reported *"repo hygiene clean"* and two critics found it red; both were right, at different times |
| [H-41](#h-41) | medium | FIXED | 7 | sweep: `deposit` | `chain-agreement.mjs`, `token-census.mjs`, `token-allowlist.mjs` | both `deposit` shots were filed UNVERIFIED on every run: the `18` of a player-protection notice read as an unasserted money figure, and T-30's new `0.0004` wallet requirement asserted by nothing. The money figure is asserted now, not allowlisted |
| [L-02](#l-02) | medium | FIXED | 5 | sweep: `lobby` How-it-works dialog | `HowItWorks.svelte` `.modal-content` | the How-it-works dialog opened **behind the disclaimer banner**: its title row and its close `×` were unreachable by mouse at 1440×900. Only the keyboard path worked |
| [L-06](#l-06) | medium | FIXED | 7 | `make hygiene` (the manifest is gitignored) | `artifacts/screens/<sha>/manifest.json` | the new occlusion pixel gate writes **60 KB per shot** into the manifest; one run's manifest is **5.58 MB**, which trips `make hygiene`'s own 4 MiB untracked-payload check. A green harness now makes a red hygiene |
| [T-07](#t-07) | medium | FIXED | 2 | `dev.sh local-up` step [6/6] | local id mapping | `frontend` is deployed on the local network and serves the app; 17 of 18 screenshots captured through it |
| [T-09](#t-09) | medium | FIXED | 3 | sweep: `table-showdown` | `PokerTable.svelte` showdown | the villain's revealed hand rendered as two blank cards and the winning hand as `0`: an unwrapped Candid `opt` in two places. Both are unwrapped now (`revealedHole()` and `handRankWords()`), and a revealed pair is lifted clear of its own plate so it can be read |
| [T-19](#t-19) | medium | FIXED | 5 | sweep: mobile viewport scale | `src/cleardeck_frontend/src/app.html` + `+page.svelte` `.header-right` | every phone renders the whole app at **0.918 scale**: `.header-right` needs 424 CSS px, the viewport meta has no `initial-scale`, so Chrome zooms the document out to fit. Every glyph is 8.2% smaller than authored and ~32 px of the screen's right edge is blank. **Fixed**: `initial-scale=1` plus a two-row table header whose `.header-right` is 374 px. Measured after: layout viewport `390x844`, `documentElement.scrollWidth = 390`, page scale `1.0000`, at 320/360/390/430/768 px wide |
| [T-23](#t-23) | medium | FIXED | 5 | `dev.sh test` → `shots-selftest test-occlusion.mjs`; sweep | `.winner-award` | the award chip covered the winner's revealed pair (**68.1% of a card's ink, 58.8% of its rank glyph**) and, on desktop, a community card's suit pip (**5.2%**). Root cause: one multiplier scaled the chip vector, which points AT the board at flank seats. The award has its own per-seat vector now; **0.0% after** |
| [T-24](#t-24) | medium | FIXED | 4 | sweep: `handhistory` mobile | `HandHistory.svelte:1318` | below 560 px the action log hid `.log-seat`, so every line on a phone read `06:03:57 AM calls` — 8 of 8 anonymous |
| [T-26](#t-26) | medium | FIXED | 4 | `dev.sh test` → `ui_limits` | `WithdrawModal.svelte:15` vs `:167,:179` | the BTC withdrawal minimum the modal **stated** (1,000 sats) was **90.9× the one it enforced** (11 sats), and the error string it printed was unreachable for 12–999 sats. Resolved to **11**, the canister's number, on every surface: a UI-only floor of 1,000 would have trapped any balance below it. Gated by `tests/money_safety/tests/ui_limits.rs`, which reads `lib.rs` and both modals |
| [T-28](#t-28) | medium | FIXED | 4 | `dev.sh test` → `ui_limits` | `WithdrawModal.svelte` `setMaxAmount` | MAX put an amount in the box that could not be withdrawn, two ways: `toFixed(4)` rounded a 123,456,789 e8s balance UP to 123,460,000 and the modal refused its own MAX with `Insufficient balance`; and it ignored the 100 ICP per-transaction ceiling, so 500 ICP produced a canister rejection from a button labelled MAX. Floors and clamps now |
| [T-29](#t-29) | medium | FIXED | 4 | `dev.sh test` → `ui_limits` | `WithdrawModal.svelte` vs `lib.rs:47,51,54` | two limits the canister enforces and no surface mentioned: the **100 ICP / 0.1 BTC per-transaction ceiling** and the **60-second withdrawal cooldown**. A player met both as an unexplained rejection. Both now stated from the mirrored constants; the ceiling is checked client-side too |
| [T-30](#t-30) | medium | FIXED | 4 | `dev.sh test` → `ui_limits` | `DepositModal.svelte:399,634` | the "you have enough to deposit" test was `balance > minDeposit`, but an ICRC-2 deposit costs the depositor **two** ledger fees, so the real floor is `minDeposit + 2 × fee`. A wallet with 1,005 sats was shown a form whose every possible deposit the ledger would refuse |
| [T-34](#t-34) | medium | FIXED | 5 | `dev.sh local-up` → `up_wire` reads the wiring back off both canisters | history canister `4xhad-gd777-77775-aaacq-cai`; `MAX_HAND_HISTORY_ENTRIES = 100`; `reset_table` | the "permanent hand history" canister is deployed, **authorised for no tables, holding zero records**, and no table is wired to it (`get_history_canister() = null` on `table_2`). Proofs live only in the table, capped at 100 hands, pruned, and wiped by one admin call |
| [T-39](#t-39) | medium | FIXED | 9 | `npm run build:mainnet` verifier (13 static checks) | `src/cleardeck_frontend/src/lib/auth.js` | local sign-in navigated to `http://undefined.localhost:4943`: `import.meta.env.CANISTER_ID_*` does not exist in this build, and Vite only exposes `VITE_`-prefixed variables |
| [H-08](#h-08) | low | FIXED | 1 | `tools/shots/package.json` declares `@dfinity/*` | `tools/shots/package.json` | `@dfinity/*` were undeclared dependencies |
| [H-21](#h-21) | low | FIXED | 3 | `dev.sh test` → `invariants::classifier::the_pinned_channel_is_read_from_rust_toolchain_toml` | `tests/money_safety/src/wasms.rs`, `tests/settlement/src/wasms.rs` | the harness's `cargo build` inherited `RUSTUP_TOOLCHAIN`, which overrides `rust-toolchain.toml`; a different toolchain produced a different module hash from identical source. Both harnesses now scrub the whole family and SET the channel read from `rust-toolchain.toml`, and print it beside the sha256 |
| [T-02](#t-02) | low | FIXED | 2 | `npm run build:mainnet` verifier (each mainnet id appears once, as display text) | "Verify Code" panel | the copy payload now comes from the same config as the wiring; each mainnet id appears exactly once in the bundle, as display text |
| [T-12](#t-12) | low | FIXED | 3 | sweep: `table-sidepots` | `PokerTable.svelte:769` | the MAIN pot was labelled `Side 1`, and on a single-layer pot it printed `SIDE 1 0.40` directly under `TOTAL POT 0.40` |
| [T-27](#t-27) | low | FIXED | 5 | sweep: `shuffleproof` | `.equity-method` inside `.pot-display` | the line that names the equity method disappeared exactly when the equity became a verdict, because the pot display is replaced by the winner banner. It is one snippet rendered into whichever readout is on screen; measured in the same frame as the badges at both viewports |
| [E-44](#e-44) | high | FIXED-NO-GATE | 5 | none | `verify_shuffle : (text, text) -> (bool) query` | the one on-chain call a non-technical player would reach for to check they were not cheated **answers "false" for a genuine proof** when the two hex strings are passed in the order a reader would pick. No parameter names in the Candid, `bool` return, so *"you called it backwards"* and *"you were cheated"* are the same answer |
| [E-58](#e-58) | high | FIXED-NO-GATE | 7 | none | `scripts/verify-build.sh --local` vs `scripts/dev.sh local-up` | the project's own verifier could not verify the project's own canisters: the documented pair of commands gave **6 of 6 MISMATCH**. `local-up` installs a native host build; `--local` rebuilt in the linux/amd64 container and compared. Those are never byte-identical, by design. The metadata the modules carry (`git:revision`, `git:dirty`) was a second, independent cause: the verifier always built with the verifier's HEAD, so a deployment one commit old mismatched for a reason that had nothing to do with the code |
| [D-01](#d-01) | low | FIXED-NO-GATE | 2 | none | `tools/differential/README.md` | `rs_poker`'s lineage is OMPEval, not "its own tables", and 5.0.0 is two months old |
| [D-02](#d-02) | low | FIXED-NO-GATE | 2 | none | wave-1 claim lists | several published numbers are unsupported; itemised below |
| [T-13](#t-13) | low | FIXED-NO-GATE | 3 | none | 3 of 4 dialogs | Escape closed `HowItWorks` and silently did nothing in `DepositModal`, `WithdrawModal` and `HandHistory`; each carried a keydown handler on a `tabindex="-1"` backdrop that nothing can focus |
| [T-17](#t-17) | low | FIXED-NO-GATE | 3 | none | `HandHistory.svelte` download button | the per-hand JSON export threw on the same BigInt class as T-10; fixed here, but it was never on any screen the harness photographs |
| [D-05](#d-05) | low | BY-DESIGN | 4 | — | `DepositModal.svelte` native-BTC path | the **10,000 sat** minimum and **~2,000 sat** cost the native-BTC flow states are the ckBTC **minter's**, and no constant in this repository enforces either, so `ui_limits.rs` cannot check them. Reduced to two named constants so they cannot disagree with each other; they can still disagree with the minter |
| [E-12](#e-12) | low | BY-DESIGN | 7 | `dev.sh test` → `deposit_subaccount_anchor::dust_below_the_fee_is_accounted_for_and_recoverable_by_topping_up` + `deposit_surface::dust_at_the_published_address_is_visible_and_recovered_by_topping_up_the_same_address` — **both RUN as of wave 11** | `claim_external_deposit` | dust at or below the transfer fee cannot move alone (arithmetic), but it is visible on every surface and recoverable by topping the same address up. Wave 11 re-drove it through the 64-hex address a player is given, and wired the target that had been outside every gate since wave 8 |
| [H-10](#h-10) | low | BY-DESIGN | 1 | — | `tools/differential` | `cargo test --release -- --ignored` exits non-zero **by design**; it was published as a repro command |
| [H-15](#h-15) | low | BY-DESIGN | 1 | — | `tools/differential` | the third reference evaluator is OFF unless `CLEARDECK_PHE_PYTHON` is set |

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
### E-01 — critical — every showdown destroyed all post-flop money — STATUS: FIXED (wave 2)

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
| `reg09` forced settlement by one timeout | 200,000,000 destroyed | 0 destroyed, and since wave 5 the hand is **not ended early either**: [E-06](#e-06) is closed. In wave 6 `reg09` was inverted from pinning that defect to gating its fix, and renamed `reg09_one_timeout_folds_only_the_seat_on_the_clock` |

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
### E-02 — FUND-THEFT — a real transfer was credited twice and the excess withdrawn — STATUS: FIXED (wave 2)

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
### E-03 — high — `state.pot` overrode the contributions in both directions — STATUS: FIXED (wave 2)

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
### E-04 — high — `notify_deposit` could never credit, and the ICP sent was stranded — STATUS: FIXED (wave 2)

**Status** **FIXED 2026-08-04**, together with [E-02](#e-02) and in that order. Fixing this alone
is what opened the E-02 theft path, and it did in fact happen for the length of one wave: see
E-02 and the status block at the top of
[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md#finding-10).

**RE-VERIFIED 2026-08-06 (wave 11)** after a third auditor raised it again as *"the path cannot
succeed"*. It succeeds: `notify_deposit(alice, block)` credits 1 ICP sent to the canister's main
account in full, and `notify_deposit(bob, block)` on the same block is refused. BOTH bugs were
reverted separately in a `cp -Rc` copy and both turn a gate red -- BUG A loudly
(`CandidDecodeFailed`, 6 tests), BUG B silently (`"Transaction is not a transfer"`, caught by
`deposit_surface::money_at_the_shared_main_account_is_recoverable_by_its_sender_and_by_nobody_else`).
The endpoint was NOT removed: it is the only door to money at the canister's shared MAIN account,
which carries no name, so a block index is the only evidence of ownership such a transfer has.
What was removed is the claim that it is a deposit path -- see
[SECURITY-FINDINGS.md FINDING 34](SECURITY-FINDINGS.md#finding-34).

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
**Wave 11 narrowed that radius at the other end**: `get_deposit_address()` no longer names the
shared main account, so following the published interface now lands the money at the caller's own
deposit address, where `claim_external_deposit()` reaches it with no block index at all. See
[SECURITY-FINDINGS.md FINDING 34](SECURITY-FINDINGS.md#finding-34).

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
### E-05 — high — a seat vacated mid-hand orphaned its stake — STATUS: FIXED (wave 2)

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
### E-06 — high — one action timeout ends the hand for everybody — STATUS: FIXED (wave 5)

**Fixed by the [E-32](#e-32) fix**, which is the same mistake: participation in the betting round
was gated on `status == Active` while eligibility for the pot was not. E-06 is what that looks
like on a table whose disconnect threshold and action clock coincide; E-32 is what it looks like
where they do not. Both are closed by the one change.

Two things now stop it. Participation is decided by `is_in_hand` / `can_still_act`, so marking a
seat `Disconnected` removes it from nothing; and `DISCONNECT_TIMEOUT_SECS` is 90, which is longer
than every deployed action clock (30 / 45 / 60), so the two thresholds cannot race at all any
more -- the ACTION clock is always first, on every table.

`reg09` in `tests/money_safety/tests/regressions.rs` pinned the broken behaviour and, from wave 5
until the wave-6 coherence pass, FAILED — which is exactly what its own comment asked for:

```
assertion `left == right` failed: ONE timeout ended the hand for everybody. Live players
before: 3. If this now leaves the hand in progress the interaction has been fixed
  left: Turn            <- the hand is still in progress. E-06 is closed.
 right: HandComplete
```

> **WAVE 6: THAT IS DONE. `reg09` NOW GATES THE FIX INSTEAD OF PINNING THE DEFECT.**
>
> It was red for a whole wave, and `./scripts/dev.sh test` was red with it, which is the worst
> possible state for a gate: a suite everybody has learned to expect one failure from cannot
> convict the second. It is renamed `reg09_one_timeout_folds_only_the_seat_on_the_clock` and now
> asserts, on the identical sequence, that
>
> 1. exactly ONE seat is folded and it is the one on the clock (`PlayerTimedOut(_)`),
> 2. the hand is still in progress with two live seats,
> 3. the board is 4 cards, not 5 — the remaining board is NOT dealt in one message,
> 4. nobody is marked `Disconnected` by a 31 s lull at all (the 90 s threshold),
> 5. nothing is destroyed, and
> 6. once every clock has expired the last seat standing wins by FOLD-OUT on the short board,
>    paid the whole 206,000,000 e8s it collected, with no manufactured showdown.
>
> **It goes RED with the defect restored.** Verified by reverting both halves in a `cp -Rc` copy
> (`DISCONNECT_TIMEOUT_SECS` 90 → 30 and `is_in_hand` back to `!p.has_folded && p.status ==
> PlayerStatus::Active`): `PINNED FIX (E-06): one timeout must not end the hand for everybody.
> Live players before: 3, phase after: HandComplete`. [SECURITY-FINDINGS.md](SECURITY-FINDINGS.md)
> FINDING 12 was updated in the same change, as `reg09` instructed.

The original finding follows.

#### As found

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
### E-07 — critical — one controller call destroyed every seated player's chips — STATUS: FIXED (wave 7)

**Where** `reset_table` AND `admin_reinit_table`, both → `init_table_state`,
`src/table_canister/src/lib.rs`. [SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 07.

**What was wrong** The two functions had byte-identical bodies — `require_controller`,
`validate_config`, `init_table_state` — and `init_table_state` builds a brand new `TableState`
with empty seats. Every seated player's chips and the whole pot were dropped: not returned to
escrow, not withdrawable, and `admin_restore_balance` had been deliberately removed, so not
recoverable by a controller either. The Candid described `admin_reinit_table` as *"for recovery
after upgrade issues"*, i.e. the interface pointed an honest operator at the button that deletes
everybody's money. Raised from high to **critical** in wave 6 when a second independent auditor
found it from the outside and ranked it their number-one blocker.

**Measured on the running local replica, 2026-08-05, BEFORE the fix** (`table_3`, min buy-in
20 ICP, two seats):
```
T2 seated              ledger_main=12000000000  escrow=0  chips=4000000000  TOTAL_CLAIMS=4000000000
reset_table            -> Ok
T3 after reset_table   ledger_main=12000000000  escrow=0  chips=0           TOTAL_CLAIMS=0
admin_reinit_table     -> Ok  (nothing left to destroy)
cash_out -> Err("Not at table")
withdraw -> Err("Insufficient balance. Have: 0.0000 ICP, requested: 20.0000 ICP")
```
40.00000000 ICP destroyed by one call, with the canister still holding it on the ledger.
`table_3` on this machine still holds 120.00000000 ICP orphaned this way — 80 from the auditor's
run, 40 from this one — because there is no restore path and there never was.

**The fix** The two doors are no longer the same function, and the destructive helper guards
itself:

* `reset_table` is a CONFIG operation and REFUSES while the table holds chips or a pot, naming
  the recovery call in the error. It does not require escrow to be empty — `BALANCES` is a
  separate map this path provably never writes, gated by
  `admin_custody::reset_table_never_touches_escrow`.
* `admin_reinit_table` is a RECOVERY operation: it calls `admin_return_all_chips_to_escrow`
  first, so every seated stack and every stake in the live hand goes to the escrow of the player
  who owns it (via `hand_stakes`, the same payout basis settlement uses, so a departed player's
  stake reaches THEM and not whoever took their chair), and only then rebuilds the table.
* `admin_return_all_chips_to_escrow` is the new standalone primitive. Conserving by construction:
  escrow only goes up, the ledger is not touched, and it refuses — changing nothing — if
  `state.pot` and the attributed stakes disagree, because paying out only the attributed part
  would destroy the difference.
* `init_table_state` now TRAPS if the table it is about to replace holds custody. The guard is at
  the helper, not only at the two callers, because the reason this survived four waves is that
  writing a second door was one copy-paste.

**Measured live AFTER the fix, same table, same script:**
```
T2 seated                   chips=4000000000  TOTAL_CLAIMS=54060000000
reset_table            -> Err("Refusing: reset_table rebuilds the table, and this table is
                              holding 4000000000 in seated chips ... See FINDING 07.")
admin_reinit_table     -> Ok
T4 after both               chips=0           TOTAL_CLAIMS=54060000000   <-- UNCHANGED
get_balance alice 0 -> 2000000000 ; bob 2000000000 -> 4000000000
withdraw(20 ICP) -> Ok; ledger and claims both fall by exactly 2000000000
```

**One bug in the first version of the fix, found by deploying it rather than by testing it.**
`Player::total_bet_this_hand` is cleared only by `start_new_hand`, so between hands every seat
still reports the finished hand's bets while `state.pot` is zero. `table_custody` read the stakes
unconditionally, double-counted, and the conservation guard then refused every ordinary post-hand
table. Every PocketIC test written for the fix was green, because none of them had played a hand
first. `admin_custody::the_admin_doors_work_on_a_table_that_has_already_played_a_hand` is the gate
that now convicts it, and it was verified to convict it by re-introducing the mutation.

**Reproduce (the gate)**
```
cd tests/money_safety
cargo test --test admin_custody
cargo test --test regressions -- reg06_admin_reinit_table_returns_every_seated_chip_to_its_owner
cargo test --test wave6_coherence -- probe5
```

---

<a id="e-45"></a>
### E-45 — high — a controller could re-denominate a funded table, making every balance unpayable — STATUS: FIXED (wave 7)

**Where** `reset_table`, `admin_reinit_table` and `admin_update_config`, all of which accept a
whole `TableConfig`. [SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 20.

**What was wrong** Found while auditing the whole controller surface for E-07's shape.
`TableConfig::currency` is not a display setting: `Currency::ledger_canister()` is what
`transfer_tokens` pays out of, and `min_withdrawal` / `transfer_fee` / `format_amount` all follow
it. Flipping an ICP table to BTC while escrow balances existed pointed every withdrawal at the
ckBTC ledger, where the canister holds nothing — so every player's money became unreachable
without a single write to `BALANCES`, and no invariant in the project measures the currency.
The reverse flip on a canister holding both assets would let sat-denominated balances be
withdrawn as ICP e8s.

**Also fixed alongside it** `admin_update_config` wrote `state.config` but not `TABLE_CONFIG`,
and `get_table_currency()` — which `withdraw` and `transfer_tokens` read — reads `TABLE_CONFIG`.
So a config change left the table charging blinds by one record and paying withdrawals by
another. Both are now written together.

**The fix** `refuse_currency_change_while_funded` on all three doors: a currency change is
allowed only when escrow + chips + pot is exactly zero.

**Reproduce**
`cargo test --test admin_custody -- currency_cannot_be_changed_while_the_canister_owes_anybody_anything`

---

<a id="e-08"></a>
### E-08 — medium — the published Candid does not describe the deployed code — STATUS: FIXED (wave 11)

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
### E-09 — medium — `poker_core` validates no input at all — STATUS: FIXED (wave 2)

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
### E-10 — medium — the withdrawal cooldown resets on every upgrade — STATUS: OPEN

`PENDING_WITHDRAWALS` and `LAST_WITHDRAWAL` are not fields of `PersistentState`, so neither
survives an upgrade. Not a fund-loss path on its own (the balance is debited before the transfer),
but a rate limit that an operator action clears is not a rate limit. Code-read; not executed.

<a id="e-11"></a>
### E-11 — medium — an upgrade mid-withdraw drops the refund path — STATUS: FIXED (wave 8)

An upgrade that lands while a `withdraw` is awaiting the ledger drops the reply callback along
with the heap, so the `Err` branch that refunds escrow can never run. If the transfer had failed,
the user's escrow stays debited with nothing delivered. **Not claimed as demonstrated:** the
harness cannot yet hold a ledger reply open across an `install_code`. Building that capability is
the test wave-2 needs here.

<a id="e-12"></a>
### E-12 — low — dust at or below the fee is stranded in a deposit subaccount — STATUS: BY-DESIGN (wave 7)

`claim_external_deposit` refuses when `balance <= transfer_fee` and no transfer can move an amount
that cannot pay its own fee. **That part is arithmetic and is unchanged.** The two parts that were
defects are closed:

* it is no longer silent. Dust is reported by `get_custody_status.unswept_deposit` (and counted in
  `total`), by `get_deposit_custody()`, by `admin_get_deposit_custody()` and by `total_liability()`;
* it is no longer misleading, and it is recoverable. The refusal states the amount, the fee, that
  the money is **not lost**, and the exact top-up to the SAME address that makes the whole balance
  claimable. Driven end to end: 9,999 e8s of dust + 2 ICP swept out together.

Gates: `tests/money_safety/tests/deposit_subaccount_anchor.rs`
`dust_below_the_fee_is_accounted_for_and_recoverable_by_topping_up`,
`the_claim_refusal_may_not_say_there_is_nothing_when_there_is`, and (wave 11)
`tests/money_safety/tests/deposit_surface.rs`
`dust_at_the_published_address_is_visible_and_recovered_by_topping_up_the_same_address`.
See docs/SECURITY-FINDINGS.md FINDING 11 and FINDING 28.

**Wave 11: re-verified through the door a player is actually given, and the gate was wired.**
The wave-8 evidence funded the subaccount with `icrc1_transfer` to
`Account { owner, subaccount }`. A player is given 64 hex characters, which are only spendable
through the ICP ledger's LEGACY `transfer` method, and nothing in this project had ever called
it. Re-driven that way the arithmetic is identical: 9,999 e8s refused with the amount named,
`get_deposit_custody().observed_amount = 9999`, then a 2 ICP top-up to the SAME address sweeps
`199_999_999`. **The gap was in the gate, not the fix**: `deposit_subaccount_anchor` was
cargo-auto-discovered and named by nothing -- not `tests/money_safety/Cargo.toml`, not
`scripts/dev.sh test` -- so this entry's only evidence sat outside every target anybody runs
([H-45](#h-45)). Both targets are named in both places now.

<a id="e-13"></a>
### E-13 — low — `detect_straight` prefers the wheel over a better straight — STATUS: OPEN

`detect_straight(&[14,6,5,4,3,2]) == Some(5)`, should be `Some(6)`. The A-2-3-4-5 check runs before
the descending window scan. Unreachable today: `evaluate_five_cards` is the only caller and always
passes exactly five ranks, and five distinct ranks cannot contain both a wheel and a higher
straight. The trap is that passing all seven cards is an obvious-looking optimisation and would
silently misrank. **Found twice independently** — FINDING 04 (extraction wave) and the
differential harness — which is the same defect, not two.

<a id="e-14"></a>
### E-14 — low — `leave_table` has no rate limit — STATUS: OPEN

Missing the `check_rate_limit()?` that `player_action` has, on a function that (per E-05) has fund
consequences. Code-read.

<a id="e-15"></a>
### E-15 — low — dead code on a fund path — STATUS: OPEN

`poker_core::side_pots::level_pot`'s `partial_contributions` term is **provably always zero**:
every bet value is itself in the deduped `bet_levels` list, so no bet can lie strictly between two
consecutive levels. Found by mutation testing (deleting the term leaves the suite green, and the
mutant was then proved equivalent). Dead arithmetic in the side-pot split invites a future reader
to trust it.

<a id="e-16"></a>
### E-16 — critical — the provably-fair shuffle could not be verified by anyone — STATUS: FIXED (wave 2)

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
### E-30 — high — an incomplete all-in raise reopened the betting — STATUS: FIXED (wave 2)

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
### E-31 — high — an expired action timer wedged the table — STATUS: FIXED (wave 2)

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
`admin_reinit_table`, which E-07 showed stranded every seated player's chips. (E-07 is FIXED as of
wave 7: `admin_reinit_table` now returns every chip to its owner's escrow before it rebuilds
anything, so it is a real recovery tool rather than the thing that loses the funds. The argument
below stands regardless — a table no message can move is still the state to avoid.) Refusing an
action without resolving it is not a conservative choice, it is the unrecoverable one.

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
both still pass, i.e. the extraction did not change what `check_timeouts` does. (`reg09` fails as of
wave 5 -- not because of the E-31 extraction, but because [E-06](#e-06)/[E-32](#e-32) were fixed and
the hand it pins is no longer ended early.)

<a id="e-32"></a>
### E-32 — high — a `Disconnected` seat is skipped by the betting round yet stays live in the hand — STATUS: FIXED (wave 5)

**Status: reproduced on the running canisters, quantified, then fixed.**

**THE CLASS.** PARTICIPATION -- whose turn is it (`find_next_active_seat`), is anybody left to play
for the pot (`count_active_players`), can anybody still put money in (`count_players_can_act`), does
the street close (`is_betting_round_complete`) -- was decided by `status == PlayerStatus::Active`.
ELIGIBILITY -- who may WIN the pot (`live_claims`) -- is decided by "seated, holds cards, has not
folded", and says nothing about `status`. Every gap between those two sets is a way to be paid for a
pot you did not pay into, or to be charged for one you cannot win.

**Every door into the gap, and what each is now.**

| door | what it did | now |
|---|---|---|
| `check_timeouts` marks a seat `Disconnected` after a lull | dropped it from the betting round, kept it eligible | `Disconnected` decides nothing; the seat is still offered the action |
| `sit_out()` mid-hand | set `SittingOut` with **no phase check** -- the same exploit with no wait at all | deferred to the end of the hand (`is_sitting_out_next_hand`), which is what real rooms do |
| `find_next_active_seat` | never offered the action to a non-`Active` seat | offers it to any seat that `can_still_act` |
| `count_active_players` | could reach 1 while TWO seats held cards, so `end_hand_single_winner` gave the pot away | counts every seat `is_in_hand` |
| `count_players_can_act` | dropped below 2 and ran the board out (that is [E-06](#e-06)) | counts every seat that `can_still_act` |
| `is_betting_round_complete` | closed the street with a live seat owing money | a street cannot close while a seat that can win the pot owes anything |

Checked and left alone: every other `status == Active` test in the file is a BETWEEN-HANDS question
(`start_new_hand`'s player count, the blind and button walk, ante posting, who gets dealt cards, the
auto-deal counters, the auto-kick, `reload`). Deciding who is in the NEXT hand from `status` is
correct; deciding who is in THIS one from it was not. `check_timeouts`'s broke-player sit-out cannot
reach a live claim either: `broke_at` is only ever set in `finish_hand`, and a 0-chip seat is
auto-sat-out before the next deal.

**WHAT IT WAS WORTH.** Measured by
`the_free_showdown_was_worth_about_a_big_blind_a_hand`, which runs the engine's own `plan_payouts`
over 20,000 real deals in the situation below -- three-handed, 20 each in, a 200 bet on the flop:

```
E-32 measured over 20000 deals: the skipped seat collected the 60 main pot on 6911 of them
(34.6%), worth 19.64 chips a hand = 0.98 big blinds a hand, against a certain -20 for folding.
```

Playing by the rules costs you 20 with certainty. Not heartbeating costs you 20 only when you lose,
and pays 40 when you win. It is a **free roll**, it is strictly better than folding in every hand it
applies to, and 0.98 BB/hand is roughly twenty times a good player's whole edge.

**REPRODUCED ON THE RUNNING LOCAL CANISTERS** (`table_2`, 45 s clock, before the fix):

```
hand 3   ONE action recorded for the entire hand -- seat 3 Fold, forced by the clock -- and a
         complete five-card board dealt. Seats 2 and 4 were both carried to a showdown having
         put in nothing but their blinds and having acted not at all. Seat 2 collected.
         (This is the auditor's evidence shape, reproduced.)
hand 2   seat 4 stopped heartbeating, was marked Disconnected with 10,000,000 e8s in the pot and
         has_folded = false, and the pot was then handed to seat 0 by end_hand_single_winner --
         no showdown -- while seat 4 still held cards that could have won it. The SAME defect
         robbing the honest player.
hand 5   seat 4 called sit_out() on the flop. Instant, one message, no 30-second wait. Never
         asked to match a 2 ICP bet, kept its stack, still has_folded = false with cards at the
         showdown.
```

**THE RULE, AND THE SOURCE.** Poker gives a player facing a bet three options -- call, raise, fold
-- and no fourth. Robert's Rules of Poker (Ciaffone), the source the wave-4 audit already recorded
against this defect: a player called upon to act who fails to act has a folded hand. Every online
room implements exactly that -- your clock runs whether or not your client is connected, and the
hand is folded when it expires. The all-in-style "disconnect protection" a few rooms offered in the
early 2000s, which capped a dropped player at what they had already contributed while they kept the
rest of their stack, was withdrawn because players triggered it deliberately. That cap is precisely
what this engine was handing out by accident, to anyone who closed their laptop lid.

**WHY NOT SIMPLY FOLD A DISCONNECTED PLAYER.** Because that punishes the honest one, and the brief
asks about them. The rule implemented is the room rule and it is strictly gentler: a dropped
connection costs **nothing at all** until the player's own action clock expires. The street cannot
close behind their back, so a pot they have chips in cannot be given away while they are still in
it; they get the full `action_timeout_secs` plus their time bank to come back; and if they do come
back inside it they play the hand out with their equity intact. Only the clock folds them -- the
same clock that folds a player who is sitting right there and says nothing.

The heartbeat threshold itself, which is all a flaky connection trips, went from 30 s to 90 s and
now governs only the "disconnected" badge, exclusion from the NEXT deal, and the start of the
120 s auto-kick clock: 3.5 minutes before a seat is given away, not 2.5.

**COST TO THE TABLE.** A dead client now holds the table up for one action clock per hand instead of
being silently skipped. That is what a real room does, and `MAX_TIMEOUTS_BEFORE_SITOUT` already
bounds it: the first timeout folds them, the second sits them out.

**VERIFIED ON THE RUNNING CANISTER AFTER THE FIX** (`table_1`, redeployed):

```
sit_out() called on the flop  ->  status stayed Active, seat stayed in the hand,
                                  and the sit-out took effect at the START OF THE NEXT HAND
seat 0 marked Disconnected    ->  seat 1 bets 1 ICP
                                  phase STAYS Flop, action_on = 0. The street did not close.
seat 0's clock then expires   ->  has_folded = true, board still 3 cards, uncalled bet returned,
                                  seat 1 wins by fold-out. No free run-out, no free showdown.
```

**Tests** `src/table_canister/tests/betting_rules.rs` sections 4 and 5. All five go RED with the
fix reverted and the 25 pre-existing tests stay green:

```
cargo test -p table_canister --test betting_rules
  a_seat_that_stops_responding_cannot_win_a_pot_it_declined_to_match
  a_seat_that_sits_out_mid_hand_does_not_leave_the_betting_round_it_is_in
  a_pot_is_not_given_away_while_a_seat_that_stopped_responding_still_holds_cards
  a_seat_that_comes_back_inside_its_own_clock_plays_the_hand_out
  every_seat_the_engine_waits_for_is_a_seat_that_can_win_the_pot
  the_free_showdown_was_worth_about_a_big_blind_a_hand
```

and at the canister level, `reg09` -- which pinned the broken behaviour -- now fails with
`left: Turn, right: HandComplete`. See [E-06](#e-06).

**STILL OWED, in files this agent does not own:** `reg09` and
[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 12 must be updated together, as `reg09`'s own
comment instructs; and this belongs in SECURITY-FINDINGS.md as a way to take another player's money.

#### As found

**Where** `is_betting_round_complete`, `count_active_players` and `find_next_active_seat` (all
require `status == PlayerStatus::Active`) versus `determine_winners` (evaluates every player with
`!has_folded`). Set up by the hardcoded 30 s `DISCONNECT_TIMEOUT_NS` in `check_timeouts`.

**Status when found: characterised, NOT fixed** ("fixing it decides who is eligible for which pot,
which is the payout path another agent owns in this wave"). It is fixed now, without touching the
payout path: the payout path was always right, and it is the BETTING path that was asking a
different question.

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

**Reproduce (historical)** `characterises_a_disconnected_player_being_skipped_yet_left_live_in_the_hand`
asserted TODAY'S wrong answer so it could not change silently. It has been REPLACED by the six tests
listed above, which assert the right answer and go red the moment the fix is reverted. Note that the
old characterisation would NOT have caught the fix on its own: it built its quiet seat with
`hole_cards: None`, a state no deal can produce, and such a seat is correctly not in the hand. That
is the standing lesson again -- the marker for a defect was blind to the defect being fixed.

**Also worth an entry in [SECURITY-FINDINGS.md](SECURITY-FINDINGS.md)** — it is a way to take
another player's money, not the canister's. STILL NOT WRITTEN THERE: that file was outside this
agent's ownership in this wave too.

<a id="e-33"></a>
### E-33 — medium — no post-or-wait-for-the-big-blind rule, so free hands can be farmed — STATUS: OPEN

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
### E-34 — medium — no dead-button rule: a player can be skipped for the big blind — STATUS: OPEN

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
  used to require `status == Active` (no longer -- see [E-32](#e-32)).** Heads-up this is right (the button is the small blind, so the big
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
### E-35 — medium — a chopped pot gave all its odd chips to one seat — STATUS: FIXED (wave 2)

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
### E-36 — **high (raised from medium in wave 6)** — a mid-hand arrival can bet into a hand it holds no cards in, AND can un-play the hand for everybody else — STATUS: FIXED (wave 6)

> **WAVE-7 COHERENCE NOTE, 2026-08-06.** The fix below is real and I re-checked all thirteen sites
> in `src/table_canister/src/lib.rs`: the predicate family is coherent and I could not make any two
> of them disagree, including the one pair that differs by design
> (`deals_in_this_hand` = `Active` vs `will_be_dealt_in` = `Active && chips > 0`), which is safe
> because `start_new_hand` sits out every broke seat before the ante loop.
>
> **It is not coherent in the PRODUCT.**
> `src/cleardeck_frontend/src/lib/components/PokerTable.svelte:212` defines its own `isInHand(p)`
> whose mid-hand arm is `!p.has_folded && variantKey(p.status) === 'Active'` — this defect's
> disjunct, hand-rolled in the client, with no cards test. It disagrees with the engine in both
> directions: it counts a cardless mid-hand arrival as in the hand, and it drops a `Disconnected`
> seat that still holds cards. It feeds `liveSeats`, so it decides which seats get equity badges,
> not who gets paid. Its own doc comment gives a real reason (an auto-deal race that leaves a
> player marked `SittingOut` with cards face up) and asserts an identity — *"So 'dealt in and still
> live' is exactly `Active && !has_folded`"* — that stopped being true when the engine changed under
> it. The cited line numbers (`lib.rs:2714`, `6765-6777`) no longer point at the deal loop.
>
> **What it owes:** the client should read the same fact the wire already carries
> (`hole_cards.is_some()`), or the comment should say plainly that the client's set is deliberately
> different from the engine's and why. A predicate stated twice is how this defect happened.

> ## FIXED 2026-08-05, BOTH HALVES, IN ONE CHANGE
>
> `is_in_hand` is now `!p.has_folded && p.hole_cards.is_some()`, and `live_claims` CALLS it
> rather than restating it, so participation and eligibility are one function and cannot drift.
> `can_still_act` inherits the cards test, so `find_next_active_seat` never offers the action to
> a seat that was not dealt in: **a mid-hand arrival can no longer bet into a hand it holds no
> cards in** (this defect's first half) and **can no longer keep `count_active_players` above
> one while the real players fold** (its second half).
>
> Measured after the fix, same sequence: the fold-out winner ends on 202,000,000 against a
> 200,000,000 buy-in and the folder on 198,000,000. Before it, all three seats ended on exactly
> 200,000,000.
>
> **The two tests this defect's marker said would break DID break, on purpose, and both were
> updated in the same change:**
>
> * the `E-36` entry in `tests/money_safety/src/documented.rs` `REGISTER` is DELETED, which
>   leaves the register EMPTY;
> * the `"carries both a live stake and a departed stake"` entry in `TOLERATED_SELF_REPORTS` is
>   DELETED, which leaves that list EMPTY. The engine still writes the line — it is now an
>   untolerated tripwire, and `e36s_dual_stake_line_is_no_longer_excused_and_would_block` in
>   `classifier.rs` asserts that emitting it STOPS a run;
> * `m8_a_chair_with_two_owners_credits_each_of_them_under_their_own_name` could no longer build
>   its own premise and is rewritten as
>   `a_mid_hand_arrival_is_never_dealt_the_action_and_can_never_stake_the_hand`;
> * `m8_a_departed_stake_is_paid_to_its_owner_and_not_to_whoever_took_the_chair` was PINNING the
>   refund-everyone outcome (every principal level) and now asserts the poker-correct one.
>
> Gates: `src/table_canister/tests/hand_membership.rs` (the predicate table, every reachable
> seat shape), `probe4` in `tests/money_safety/tests/wave6_coherence.rs`, and **M11 OUTCOME** on
> every fuzz step.
>
> ---
>
> ## WAVE 6: THIS DEFECT NOW HAS A THIRD-PARTY VICTIM (kept for the history)
>
> The wave-6 write-ups say participation is now "asked of THE SAME PREDICATE as eligibility",
> and `is_in_hand`'s own doc comment says of the cardless seat that *"including it changes
> nothing; it holds no cards, so `live_claims` still refuses to pay it."* **Both sentences are
> false, and the second one is why nobody looked.** Participation is
>
> ```rust
> pub fn is_in_hand(p: &Player) -> bool {
>     !p.has_folded && (p.hole_cards.is_some() || p.status == PlayerStatus::Active)
> }
> ```
>
> and eligibility is `live_claims`, which requires `hole_cards.is_some()`. A cardless `Active`
> seat is in one set and not the other. That is exactly the disagreement E-32 was closed to
> remove, still open at a fifth site.
>
> It matters now because `count_active_players(state) == 1` triggers `end_hand_single_winner`,
> and the ONE seat it counts can be the cardless one. `live_claims` is then empty, so
> `plan_payouts` takes its wave-6 "nobody can win ANY of this money" branch and hands **every
> stake back to its funder, including the players who folded.**
>
> **Measured on the wave-6 module `5e07edf6…`** (`tests/money_safety/tests/wave6_coherence.rs::probe4`),
> and note that nothing in this sequence is hostile or even unusual:
>
> ```
> pot = 52,000,000 e8s, alice v bob heads-up
> carol calls join_table(2) mid-hand  -> Ok
> carol calls sit_in()                -> Ok
> alice folds                         -> ordinary poker; bob has WON by fold-out
> bob's client drops; his own action clock expires
> check_timeouts -> PlayerTimedOut(1)
>
> FINAL phase=HandComplete pot=0
>   seat0 chips=200000000  (its exact buy-in)
>   seat1 chips=200000000  (its exact buy-in -- the fold-out winner)
>   seat2 chips=200000000  (its exact buy-in)
> ```
>
> No trap. No `CRITICAL:` line. Conservation exact. M1 through M9 silent, the drain clean.
> **The hand was un-played: the player who folded got their blind back and the player who won
> got nothing.** Before wave 6 the same state destroyed the money instead (`plan_payouts`
> returned early and the next `start_new_hand` zeroed the pot), so this is strictly an
> improvement, but the property the wave claims -- that the table can no longer be wedged into
> a state the rules cannot describe -- is **not** established for the states that settle
> quietly. It is established only for the states that TRAP.
>
> **Not fixed in wave 6 either**, and deliberately so: the one-line change (drop the
> `|| p.status == PlayerStatus::Active` disjunct) is a behaviour change to the seating path
> during a wave with three other agents in the file, and it must be landed together with the
> register entry and the tolerated self-report below, in ONE change. It is the shortest item on
> docs/WAVE-06.md's path-to-yes.

### THE PREDICATE TABLE — every site in the engine that answers "is this seat in the hand"

Built by the wave-7 sweep that closed this defect. This is the enumeration the brief asked for:
every predicate in `src/table_canister/src/lib.rs` that answers the question in ANY sense, what
it answered BEFORE, and whether that agreed with `live_claims`. **Five sites disagreed. Four were
defects of the same shape and are fixed; one is deliberate and is now named.**

| # | site | question it answers | rule BEFORE | agreed with `live_claims`? | now |
|---|---|---|---|---|---|
| 1 | `is_in_hand` | in the hand: may win, counted for the fold-out, owed a turn | `!folded && (cards \|\| status==Active)` | **NO** — a cardless `Active` seat | `!folded && cards`. **THE predicate** |
| 2 | `live_claims` | who may be PAID | `!folded && cards`, stated separately | n/a (it is the reference) | CALLS `is_in_hand`. One rule, one function |
| 3 | `count_active_players` | drives `end_hand_single_winner` | `is_in_hand`, so inherited #1's error | **NO** | `live_claims(...).len()`, asserted |
| 4 | `can_still_act` / `find_next_active_seat` / `count_players_can_act` | owed an ACTION | `is_in_hand && !all_in`, so inherited #1 | **NO** | `is_in_hand && !all_in`, now correct |
| 5 | `leave_table`'s `was_in_hand` | did this departure change the hand | `!has_folded && hand_is_live` | **NO** — accepted a cardless seat, marked it folded, re-ran the fold-out check for it | `hand_is_live && is_in_hand(p)`, plus an explicit `was_action_on` safety net |
| 6 | `withdraw` / `cash_out`'s in-hand guard | may this player leave with their stack | `!has_folded` | **NO** — refused a cardless seat that had nothing committed | `is_in_hand(p)` |
| 7 | `is_betting_round_complete`'s BB-option case | must the street stay open for the BB | `!acted && !all_in && !folded` | **NO** — held the street open for a mid-hand arrival who had taken the BB chair | `!acted && can_still_act(p)` |
| 8 | `apply_player_action`'s acted-flag reset | whose flags to clear on a raise | `!folded && !all_in` | **NO** (harmless, but incoherent) | `can_still_act(p)` |
| 9 | `sit_out`'s defer rule | is the caller in a hand they must finish | `is_in_hand` | inherited #1 | inherited the fix |
| 10 | `hand_stakes`'s `relinquished` | did this STAKE give up its claim | `p.has_folded` | yes | unchanged. It is a question about a stake, not a seat |
| 11 | `deals_in_this_hand` (`start_new_hand`) | who is DEALT | `status == Active` | **DELIBERATELY NOT** | named, and documented: it is the only WRITER of the fact `is_in_hand` reads. No `chips > 0` test, because the blinds are posted first and can leave a seat all-in at zero, and that seat must still be dealt in |
| 12 | `will_be_dealt_in` | who takes part in the NEXT hand: can a hand start, who gets the button and the blinds, is auto-deal due | `status == Active && chips > 0`, written out inline at **ten** sites | **DELIBERATELY NOT** | named, single-sourced. Says nothing about a hand in progress and no call site may use it to decide one |
| 13 | `get_table_view` / `get_hand_history` display fields | what to SHOW | `!has_folded`, `is_showdown && !has_folded` | n/a | unchanged. Display only; moves no money |

Sites 11 and 12 are the two genuine differences, and they are differences of TENSE: they are about
the next hand and about the deal, not about the hand in progress. Everything else is now one
predicate. `src/table_canister/tests/hand_membership.rs::predicate_table` executes rows 1-4 over
every reachable `Player` shape and prints the table; the between-hands rows are covered by
`the_between_hands_predicates_say_nothing_about_a_live_hand`.

**Status** **FIXED 2026-08-05.** Found 2026-08-04 by the money-safety fuzzer at 600 steps (seed
`0xc1ea2dec0003`, 6-max with an ante) while validating the E-01/E-03/E-05 payout fix; re-measured
on `306caef4` and closed on `34f9d194`. See the block above and
[FINDING 17](SECURITY-FINDINGS.md#finding-17).

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

**This defect is now REGISTERED as shipping, and that is its marker.** `documented::REGISTER`
carries one entry, `E-36` on the check `canister_reports_its_own_inconsistency`, direction
`Unsigned`, bound **0 e8s** — see [H-28](#h-28) for why it had to exist and why the alternative was
rejected. Two consequences worth knowing before touching this defect:

* the money-safety fuzzer now COUNTS the `WARNING:` line instead of blocking on it, so
  `cargo test --test fuzz` reports `1 documented finding(s) ... x8 worst_delta=0` and carries on
  exploring past E-36 instead of stopping at it;
* **fixing E-36 broke two tests on purpose, and both were updated with it** (2026-08-05).
  `register_entries_are_all_still_needed` fails if
  the `E-36` id leaves this file while the register entry stays, and it fails if the
  `"carries both a live stake and a departed stake"` line leaves `src/table_canister/src/lib.rs`
  while `TOLERATED_SELF_REPORTS` still excuses it. Delete the defect, the register entry and the
  tolerated substring in one change.

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
### T-01 — high — a bare `npm run build` wires the bundle to MAINNET — STATUS: FIXED (wave 9)


> **CORRECTION, wave 14 coherence pass.** This row is `FIXED` and its gate,
> `npm run build:mainnet`, **exits 1 at HEAD** — that is
> [T-46](#t-46). `register-stats.sh --check` cannot see it: it verifies that a
> `FIXED` row NAMES a gate, never that the named gate passes. A gate that cannot
> pass is as uninformative as one that cannot fail. The compiled value is still
> `"ic"`; it is the DETECTOR that went stale.


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
### T-02 — low — a locally-wired build still offers a mainnet `-e ic` command — STATUS: FIXED (wave 2)

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
### T-03 — medium — the app hardcodes gateway port 4943 — STATUS: OPEN

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
### T-04 — medium — the local network state cannot be resumed — STATUS: OPEN

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
### T-05 — low — `btc_table_1` is never registered in the lobby — STATUS: OPEN

`scripts/deploy-mainnet.sh` calls `init_microstakes_tables(table_1, table_2, table_3)` and carries
a `# NOTE: verify whether btc_table_1 needs a separate lobby registration call.` `make local-up`
now states this explicitly instead of leaving it as a comment.

**DETERMINED in the wave-5 lobby pass — see [L-05](#l-05).** It is **not** a frontend fix: the
frontend already renders BTC rows completely and `lobby.get_tables()` simply does not return one.
The call is `add_btc_headsup_table(principal "<btc_table_1>")`, admin only, and the config it
registers matches `btc_table_1`'s contract exactly. L-05 carries the exact command, where the line
belongs so it survives a `local-up`, and the 35 px the extra filter row costs the phone layout.

<a id="t-06"></a>
### T-06 — low — two lists of mainnet canister ids — STATUS: OPEN

`canister_ids.json` (legacy dfx shape) and `.icp/data/mappings/ic.ids.json` (what icp-cli reads)
both list all seven mainnet canisters. They agree today. `ic.ids.json` is authoritative — it is
what `-e ic` resolves and what the mainnet guards in `scripts/dev.sh` and
`tools/shots/lib/ids.mjs` now read. `canister_ids.json` should be deleted or generated.

<a id="t-07"></a>
### T-07 — medium — the frontend asset canister has never been created locally — STATUS: FIXED (wave 2)

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
### H-01 — high — the money-safety harness is not a regression gate — STATUS: FIXED (wave 2)

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
### H-02 — high — a canister that reports its own inconsistency cannot fail a fuzz run — STATUS: FIXED (wave 2)

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
### H-03 — high — violations are classified by sign, so a house rake passes — STATUS: FIXED (wave 2)

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
### H-04 — medium — the seam is not mutation-tested — STATUS: OPEN

> ## WAVE-7 RE-RUN: **6 of 7 die. NOT DOWN.** Same survivor as waves 2, 3, 4 and 6.
>
> Script `$SCRATCH/seam_mutations_w7.py`, run against a `cp -Rc` copy, one mutation at a time,
> restored between each, every anchor asserted to match EXACTLY ONCE, a mutation that fails to
> compile reported `NOT-APPLIED` rather than counted as a kill, the wasm rebuilt per mutation so the
> money-safety harness loads the mutant. `lib.rs` sha256
> `93c5c1f9852d07e293ebcf561d73081ac39af82492eee8d3bfb60eb0836cb505` before **and after**, identical.
> Gate per mutation: `cargo test --workspace` plus
> `cd tests/money_safety && cargo test --test invariants`.
>
> | mutation (wave-7 site) | verdict | what convicted it |
> |---|---|---|
> | 1. `refresh_side_pots` made a no-op | **died** | money_safety `invariants` |
> | 2. every stake halved in `plan_payouts` | **died** | `cargo test --workspace`, money_safety `invariants` |
> | 3. the breakdown written into a throwaway `Vec` | **died** | money_safety `invariants` |
> | 4. `&[]` as the contributions in `plan_payouts` | **died** | `cargo test --workspace`, money_safety `invariants` |
> | 5. the self-report line dropped | **SURVIVED** | — (equivalent mutant, unchanged since wave 2) |
> | 6. `try_evaluate_hand` stubbed to `RoyalFlush` on the settlement path | **died** | `cargo test --workspace`, money_safety `invariants` |
> | 7. every hand dealt from `b"CONSTANT"` | **died** | money_safety `invariants` |
> | | **6 of 7** | same survivor |
>
> Two anchors re-translated for this tree, keeping what each mutation MEANS: mutation 2 now halves
> the `Vec<Stake>` `plan_payouts` takes from `hand_stakes` (wave 6's line is now shared with
> `refund_every_stake`, so the wave-6 anchor matched twice); mutation 6 stays on
> `poker_core::try_evaluate_hand` inside `rank_claims`, the only evaluator call the FINDING 15 fix
> leaves on the settlement path.
>
> **What the count does NOT say.** Every one of these seven mutations targets the PAYOUT ARITHMETIC.
> None of them can see [E-59](#e-59), because E-59 moves no money wrongly through `plan_payouts` at
> all — it chooses a different, perfectly conserving settlement. A mutation suite aimed at the seam
> is not an instrument for which settlement runs.

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
> **Wave 4 re-measurement, and what it does and does not say.** The wave-4 attribution work is
> purely ADDITIVE to the gate — four new invariant legs, three new hand-written tests, one new
> column in the settlement comparator, two new build-pin tests; no assertion, no test, no
> `documented::REGISTER` entry and no `TOLERATED_SELF_REPORTS` line was removed or loosened — so no
> mutation that died can have started surviving. **The count cannot have gone down.**
>
> Mutations **5, 6 and 7** were re-applied against the CURRENT tree (not the `801aa79` baseline the
> table above uses) and all three DIED: 5 and 6 at `cargo test --workspace`, 7 at both targets.
> Mutation 5 dies here where it survived on `801aa79` because the current `poker_core` pins the
> `BUG: Side pots` string in its own unit test; against the wave-1 baseline it still has no
> reachable trigger through the canister. Mutations 1-4 target `calculate_side_pots`, which the
> wave-2/3 rewrite moved off the payout path into `poker_core::side_pots`' archive section, so they
> are only meaningful against the `801aa79` baseline and that baseline was not rebuilt in this pass.
>
> The wave-4 pass adds **six more mutations of its own** — conserving misdirection bugs on the
> payout path — and all six die. They are a better instrument than 1-4 for the code as it now
> stands, and the matrix is
> [here](#attribution-what-was-planted-and-what-convicted-it).
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
### H-05 — medium — the differential harness fabricates its oracle for degenerate inputs — STATUS: FIXED (wave 2)

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
### H-06 — low — two of the six host-level money-safety tests do not test the canister — STATUS: OPEN

`the_ledger_anchored_invariants_are_not_checked_here` only asserts a `Cargo.toml` exists, and
`a_consistent_table_satisfies_every_host_checkable_invariant` checks the oracle against itself.
When the critic broke the real `collect_contributions`, only 1 of the 6 went red.

<a id="h-07"></a>
### H-07 — an unverified scene wrote the canonical PNG filename — STATUS: FIXED (wave 1)

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
### H-08 — undeclared dependencies in the screenshot harness — STATUS: FIXED (wave 1)

`tools/shots/package.json` declared only `playwright`; every `@dfinity/agent`, `@dfinity/identity`
and `@dfinity/principal` import resolved by Node walking **up** to the repo-root `node_modules`.
An `npm ci` inside `tools/shots`, or a pruned root, broke all 20 modules. Fixed: the three
packages are declared at `^3.4.3` (matching the frontend) and installed locally.

<a id="h-09"></a>
### H-09 — medium — an unguarded layout race in the scenes being photographed — STATUS: FIXED (wave 2)

At t=1 s the lobby renders the `Loading tables…` spinner block **and** the populated/empty lobby
simultaneously; by t=3 s the spinner is gone. That block is ~230 px of vertical layout, so whether
it appears in a given PNG is a coin flip on timing. `FREEZE_CSS` kills animation, not this state
overlap. No scene waits for `.loading-state` to clear.

**Wave 2** every scene should assert `.loading-state` is absent before shooting.

<a id="h-10"></a>
### H-10 — low — a published repro command that exits non-zero by design — STATUS: BY-DESIGN (wave 1)

`cd tools/differential && cargo test --release -- --ignored` is 1 passed / 5 **failed**. The five
failures are the E-09 and E-13 markers, each with `un-ignore when the fix wave lands` in its
reason string: honest in the code, misleading in a claim list. `make known-defects` now inverts
them properly — it succeeds while the defects are present, distinguishes "ran 0 tests" from
"fixed", and shouts when one goes green.

<a id="h-11"></a>
### H-11 — medium — the fuzzer never exercises the deployed `table_2` / `table_3` shapes — STATUS: OPEN

`tests/money_safety/src/table_api.rs:80-108`: `six_max_icp()` uses **`table_1`'s** blinds
(0.01/0.02) with `max_players: 6`, and `heads_up_icp()`/`six_max_with_ante()` both derive from it.
So every fuzz run uses `action_timeout_secs: 30`. That happens to be the E-06 danger value, but it
means the harness structurally cannot see the 45 s/60 s headroom that distinguishes `table_2` and
`table_3`, nor their 0.05/0.10 and 0.10/0.20 blind levels, nor 9 seats. Three copies of the table
config now exist (`icp.yaml`, `tools/shots/lib/config.mjs`, `table_api.rs`) and only `icp.yaml` is
authoritative.

<a id="h-12"></a>
### H-12 — medium — nothing proves the right player won — STATUS: FIXED (wave 4)

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
### H-13 — low — SplitMix64 implemented twice — STATUS: OPEN

`tools/differential/src/rng.rs` (`SplitMix64`) and `tests/money_safety/src/rng.rs` (`Rng`) share
the same core constants but differ in `below()`: rejection sampling versus raw modulo, and panic
versus 0 on `n == 0`. Not consolidated in this pass on purpose — unifying `below()` would change
the money-safety fuzzer's stream and invalidate every recorded seed. If they are merged, both
variants must be kept under distinct names so existing seeds still reproduce.

<a id="h-14"></a>
### H-14 — medium — `docs/DESIGN-BAR.md` would reject correct work — STATUS: FIXED (wave 5)

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
### H-15 — low — the third reference evaluator is off by default — STATUS: BY-DESIGN (wave 1)

`cargo run --release` in `tools/differential` prints `# auditing the two references against
phevaluator …` and then `NOTE adjudicator skipped: CLEARDECK_PHE_PYTHON not set`. The headline
"three independently-implemented mature evaluators" needs that variable. `make phe-venv` now
installs it (verified working on Python 3.14.3) and `make diff-full` sets it, so the default path
runs all three.

---

## Documentation and evidence

<a id="d-01"></a>
### D-01 — `rs_poker`'s provenance is overstated — STATUS: FIXED-NO-GATE (wave 2)

`tools/differential/README.md` calls reference A's evaluator "its own perfect-hash tables,
generated from scratch in the crate's `build.rs`". That `build.rs` says, line 4: *"The algorithm is
reimplemented from zekyll's OMPEval (MIT)."* The lineage is OMPEval. It is still genuinely
independent of the Cactus-Kev/treys lineage that reference B ports, so the independence argument
survives — but 5.0.0 was published 2026-06-09, so the crate's 88k lifetime downloads belong to nine
years of history, not to these two-month-old tables. "Mature, widely used" is doing more work than
the evidence supports for reference A specifically. `phevaluator` corroborates, which is why H-15
matters.

<a id="d-02"></a>
### D-02 — unsupported numbers in the wave-1 claim lists — STATUS: FIXED-NO-GATE (wave 2)

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
### E-37 — high — a departed player's refunded stake is paid to whoever took their chair — STATUS: FIXED (wave 3)

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
### E-38 — high — two persisted fields that are not Candid-compatible additions, one of them silent — STATUS: FIXED (wave 3)

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
### T-08 — high — the table displayed the pot at twice its size during every betting round — STATUS: FIXED (wave 3)

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
### T-09 — medium — the showdown rendered an unwrapped Candid `opt` twice — STATUS: FIXED (wave 3)

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
### H-16 — high — the harness never upgrades across a version boundary — STATUS: FIXED (wave 3)

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
### H-17 — high — the fund-theft gate was outside the gate — STATUS: FIXED (wave 3)

`tests/money_safety/tests/deposit_replay.rs` holds the E-02 theft reproducer and the ten
regressions that keep it shut. It is a cargo **auto-discovered** test target, and for the whole of
wave 2 no `scripts/dev.sh` target named it: `cmd_test` ran `--test invariants`, `--test
regressions` and `--test fuzz` only. The project's only proven fund-theft primitive had its
regression suite run by nobody.

Now named explicitly in `cmd_test`, so a rename fails the gate rather than silently removing it.

<a id="h-18"></a>
### H-18 — high — `dr02` never presents the deposit block it is named after — STATUS: FIXED (wave 5)

`MAX_DEPOSIT_VERIFICATIONS_PER_MINUTE` is 5. `dr02`'s sweep loop advances the clock by 61 s when it
is rate-limited but then moves to `b + 1` **without retrying `b`**. In that fixture the ICRC-2
deposit block is deterministically 5 — the 6th iteration, the one that gets rate-limited. So the
named regression on the double-credit primitive skips the only index that matters.

Proved by reversal by the deposit reviewer: with both defences removed, `dr02` still reported `ok`;
adding a four-line retry to the identical build made it fail with `DOUBLE CREDIT (E-02) … Deposit
block was 5`. `dr09` is the only real gate on that mechanism today.

**Fix** retry the same index after advancing the clock. Four lines.

<a id="h-19"></a>
### H-19 — medium — the register's own safety mechanism does not exist — STATUS: FIXED (wave 5)

`tests/money_safety/src/documented.rs` documents `register_entries_are_all_still_needed` as the
test that fails once a register entry stops being hit, "so a fix cannot quietly leave stale
tolerance behind". `grep -rn register_entries_are_all_still_needed tests/` returns nothing.

Currently harmless — the register is `&[]`, so there is nothing stale to police — and that is
exactly when to add it, before the first entry goes back in.

<a id="h-20"></a>
### H-20 — medium — a live defect's self-report was moved out of the detector's view — STATUS: FIXED (wave 3)

`check_self_reported_inconsistency` matches `BUG:` and `CRITICAL:` only. The payout fix wrote
E-36's dual-stake condition as `WARNING: seat {} carries both a live stake and a departed stake`,
which matches neither, so 296 occurrences of a real open defect executing against the real
canister were reported as `0 documented finding(s)` in one 1,200-step fuzz run.

This is the H-03 pathology the register exists to prevent: tolerance living in a string the
classifier ignores, where nothing can police it. Fixed by matching `WARNING:` as well and naming
that one line in `TOLERATED_SELF_REPORTS`, so it is counted, visible, and deleted when E-36 is
fixed.

<a id="h-21"></a>
### H-21 — low — the harness's build of the canister is toolchain-sensitive — STATUS: FIXED (wave 3)

`wasms.rs::build_table_canister` shelled out to `cargo` with the **inherited environment**. A
parent `cargo` exports `RUSTUP_TOOLCHAIN`, and that overrides `rust-toolchain.toml` even though the
child's `current_dir` is the repo root. Running the harness from a crate that resolves to a
different toolchain built module `7babe62428943bf9…` from source that `./scripts/dev.sh wasm`
compiles to `395696359b6d1a6e…`.

It agrees whenever the Makefile is the caller, and the harness does verify that the installed
module hash equals what it built, so H-01 is genuinely closed. But "the sha256 this harness prints
is the sha256 that gets deployed" was a property of the invocation, not of the code.

**FIXED** in both harnesses (`tests/money_safety/src/wasms.rs`, `tests/settlement/src/wasms.rs`).
`pinned_table_canister_build()` is now the only way either of them compiles the canister. It

* removes `RUSTUP_TOOLCHAIN`, `RUSTC`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, `RUSTFLAGS`,
  `RUSTDOCFLAGS`, `CARGO`, `CARGO_ENCODED_RUSTFLAGS`, and everything under `CARGO_BUILD_*`,
  `CARGO_PROFILE_*`, `CARGO_TARGET_*` and `CARGO_UNSTABLE_*`;
* then SETS `RUSTUP_TOOLCHAIN` to the channel parsed out of the tree's own
  `rust-toolchain.toml`, so the pin and the repo's pin cannot drift apart;
* prints it beside the hash: `wasm under test sha256=5996594741afd63f… toolchain=1.90.0`.

`CARGO_TARGET_DIR` mattered as much as the toolchain and was not in the original write-up: with it
set the build succeeds **somewhere else** while `build_table_canister` goes on reading and hashing
whatever stale bytes are still at `target/wasm32-unknown-unknown/release/table_canister.wasm`.
That is H-01 reachable from the environment alone.

Gated by `classifier::the_canister_build_refuses_to_inherit_a_toolchain_or_an_output_directory`
and `oracle_rules::the_canister_build_pins_its_toolchain_and_its_output_directory`, which plant
`RUSTUP_TOOLCHAIN=1.96.1`, `CARGO_TARGET_DIR=/tmp/somewhere-else`, `RUSTFLAGS=-C opt-level=0` and
four more into the test process and assert none of them reaches the command.

<a id="h-26"></a>
### H-26 — high — a fuzz run is not the pure function of its seed that it claims to be — STATUS: FIXED (wave 14)

> **FIXED 2026-08-09. The impurity was not in the replica, the process or the generator — it was
> that the TABLE SHAPE was chosen by the seed's POSITION IN THE LIST.**
>
> `tests/money_safety/tests/fuzz.rs` had `config_for(i)` where `i` is the index of the seed in
> `MONEY_FUZZ_SEEDS`, so position 0 played heads-up, position 1 played 6-max and position 2 played
> 6-max-with-an-ante — while every reproducer this harness printed, wrote into
> `money-fuzz-report.json` and got quoted by in this register named **the seed and nothing else**.
> `Reproducer` did not even have a field for the config. So `MONEY_FUZZ_SEEDS=<one seed>` replayed a
> *different game* from the one that found the violation, and this entry's own observation — seed
> `212967420072194` finds nothing alone and two fund-creation findings when `212967420072193` runs
> before it — is that, exactly and completely. Nothing about `run_sequence` was impure; the tuple
> `(seed, config, actor_names, steps)` was simply not what the driver supplied.
>
> **The fix.** One function, `fuzz::run_shape(position, seed)`, which ignores `position`. The
> mapping is `(seed - 1) % 3` and the `- 1` is chosen so that **every seed set this repository
> actually runs keeps the shape it has always had** — seed `1` (the `fast` smoke row), the three
> `0xC1EA_2DEC_000n` defaults, and the nine consecutive seeds of the deep sweep all map term for
> term onto their old positions — so every figure recorded against `fuzz-default` in this register
> stays comparable. The reproducer now records the shape and a literal replay command, and both are
> printed next to every finding.
>
> **RED before, GREEN after.** With `run_shape` reverted to `position % 3`:
>
> ```text
> assertion `left == right` failed: seed 0x0 plays a DIFFERENT GAME at position 1 than at
> position 0: heads_up_icp vs six_max_icp. A reproducer that names only the seed therefore does
> not reproduce, which is docs/DEFECTS.md H-26.
> ```
>
> **And the outside anchor, which is the one that matters.** Seed `212967420072194` at 400 steps,
> run in two separate processes — once second in the list after `212967420072193`, once alone — and
> the two run objects in `money-fuzz-report.json` are **identical**: same `config`, 400 steps
> executed, 7 hands, 6 upgrades, same `final_ledger_main`, same `final_internal_total`, same
> `transcript_tail`. The report is written by the harness and compared afterwards by something else,
> which is the only kind of agreement worth having here.

`tests/money_safety/src/fuzz.rs` opens: *"A run is a pure function of `(seed, config,
actor_names, steps)`: the generator is a SplitMix64 stream and PocketIC is deterministic, so a
failing seed replays exactly."* The first half is true. The second is not.

Measured, three ways, all on the same wasm:

```text
MONEY_FUZZ_SEEDS=212967420072194 MONEY_FUZZ_STEPS=400          -> 4 hands, 0 blocking findings
MONEY_FUZZ_SEEDS=...193,...194   MONEY_FUZZ_STEPS=400          -> seed ...194: 5 hands, 2 findings
```

Same seed, same steps, same config, same module hash; the only difference is that another `World`
was created earlier in the same test process. Consecutive PocketIC instances in one binary do not
get independent subnet randomness, so the deal — which comes from `raw_rand` — differs, and with it
the whole trajectory. Reproduced on a pristine `git archive HEAD` tree with the pristine harness,
so it is not an artifact of anything added in this wave.

Three consequences, in increasing order of seriousness:

1. **Every single-seed reproducer in this repository is unverified.** The documented workflow is to
   take the failing seed out of the report and re-run it alone. That runs a different sequence.
2. **The shrinker's output can be a fiction.** `shrink()` re-runs candidates in fresh `World`s
   inside the same process, one after another, so each candidate sees a different randomness
   stream from the run that produced the finding. A "minimal reproducer" is minimal for the
   trajectory it happened to hit while shrinking.
3. **Coverage is a lottery the report does not disclose.** `make fuzz` runs nine seeds in one
   process; seed *k*'s behaviour depends on seeds 1..*k-1*.

**Fix direction.** Either give each `World` an explicitly seeded randomness source that PocketIC
will honour, or run each seed in its own process and make the report say so. Until then a
reproducer must carry the full seed LIST and the position of the seed within it, not just the seed.
[E-41](#e-41) is the first finding this affects: it only reproduces in the two-seed form.

> ## WAVE 13 — THE COVERAGE HALF IS CLOSED. THE PURITY HALF ABOVE IS NOT, AND STAYS OPEN.
>
> [E-41](#e-41) was root-caused in wave 13, and the interesting part for this entry is that
> **the two-seed form was never the point.** The default target did not reach E-41 for a reason
> that has nothing to do with seed order or run length:
>
> 1. **The alphabet could not express the call.** `Op::UseTimeBank` names a RANDOM actor, and
>    `use_time_bank` refuses anybody who is not the seat `action_on` names. Across 220 steps at
>    three seeds it therefore essentially never LANDED — an op that is always refused looks like
>    coverage and measures nothing. Worse, the harness's ONLY seat-resolver, `on_the_clock`,
>    returns `None` unless a hand is in progress, so no op in the alphabet could reach a
>    between-hands surface with the right caller at all. E-41 lives in exactly that gap.
> 2. **Deadlines were drawn independently of the ops that arm them.** Every clock defect in this
>    register ([E-54](#e-54), [E-56](#e-56), [E-59](#e-59), [E-41](#e-41)) is "one message arms
>    something, a later deadline-crossing acts on it", and the generator had to draw that pair, in
>    that order, by luck.
>
> Both are fixed: `Op::UseTimeBankOnClock` with `under_the_action_pointer`, and a state-arming op
> that sometimes carries its own `AdvanceTime(31 s)` + `CheckTimeouts` out of the same step budget.
>
> **MEASURED, all four runs at the DEFAULT 3 seeds x 220 steps with nothing in the environment,
> `MONEY_FUZZ_SHRINK=0`, same machine:**
>
> | canister | generator | blocking findings | E-41 reached | wall |
> |---|---|---|---|---|
> | E-41 fixed | HEAD's | 1 — [E-89](#e-89), M9, 20,002 e8s | — | 74.0 s |
> | E-41 fixed | + change 1 only | 1 — M3, 10,000 e8s, same E-89 family | — | 81.3 s |
> | **E-41 UNFIXED** | **+ changes 1 and 2** | **32**, incl. `M1_CONSERVATION delta=-200000000`, `-399000000`, `-601500000`, `M9` stranding 12.49 ICP and 14.60 ICP, and `M1b_POT_BREAKDOWN` quoting the engine's own `CRITICAL: pot accounting disagreement in hand 2: state.pot = 0 but the contributions ... sum to 399000000` | **YES** | 87.8 s |
> | E-41 fixed | + changes 1 and 2 | 2 — M3 10,000 + M9 20,002, both [E-89](#e-89) family | — | **55.6 s** |
>
> **The runtime cost is negative.** 55.6 s against the 74.0 s baseline on the same box, because
> the follow-up ops come out of the same 220-step budget and a run that resolves its clocks gets
> through more hands. Nothing was lengthened and no seed was added.
>
> Three things this does NOT close, stated so nobody reads the table above as more than it is:
>
> * **The purity defect is untouched.** Consecutive `World`s in one process still do not get
>   independent subnet randomness, so seed *k* still depends on seeds 1..*k-1* and the shrinker's
>   output is still only minimal for the trajectory it hit. Both red boxes above are evidence OF
>   that, not against it: changing the generator moved the surviving E-89-family red from seed 2
>   (M9, 20,002) to seed 1 (M3, 10,000) without any canister change.
> * **It is still search.** The 32 findings are a measurement of one op stream, not a guarantee.
>   E-41's own deterministic gate is `reg39_*` and `payout_tests::e41_*`, which is where a fix
>   belongs when it must not depend on luck.
> * **The default fuzz step of `./scripts/dev.sh test` was red before this change and is red
>   after it**, on the [E-89](#e-89) / [FINDING 31](SECURITY-FINDINGS.md#finding-31) dead band.
>   That red is true and is somebody else's entry.

<a id="e-41"></a>
### E-41 — high — a settled hand settles a second time and pays itself out of nothing — STATUS: FIXED (wave 13)

> ## ✅ ROOT-CAUSED, SHRUNK AND CLOSED IN WAVE 13.
>
> **The origin is not `check_timeouts` and it is not the fuzzer's two-seed form. A HAND SETTLES
> TWICE.**
>
> `finish_hand` empties `state.pot`, clears the departed stakes, sets `HandComplete` and turns the
> action clock off. It deliberately does NOT clear `total_bet_this_hand`: that is the RECORD of what
> the hand collected, three instruments read it after the hand is over, and only `start_new_hand`
> clears it. So **between two hands the table sits on a complete payout basis over an empty pot**,
> and every piece of the settlement path is willing to act on it — `hand_stakes` still builds the
> stakes, `count_active_players` still counts the seats holding last hand's cards, and
> `plan_payouts` still produces a plan that CONSERVES, because it conserves against its own
> `collected`. `apply_payouts` therefore sees a plan that adds up and credits it. Every e8 of it is
> created.
>
> **The door was `use_time_bank`.** It asked only *"is `action_on` pointing at you"*, which is a
> question about a stale pointer between hands, and it never asked whether there was a hand. It was
> the only entry point in the file that could arm an `ActionTimer` with no hand in progress.
>
> **The shrunk reproducer: one limped heads-up hand and five ordinary player calls.** No hostile
> input, no upgrade, no fault injection, no controller.
>
> 1. alice and bob sit down and play one hand. It is limped and checked to a showdown: 4,000,000
>    e8s go in, the winner is paid, `finish_hand` closes it. Both seats still hold the cards they
>    were dealt and still carry 2,000,000 of `total_bet_this_hand`.
> 2. Whoever `action_on` still names calls `use_time_bank()`. A fresh 30-second `ActionTimer` is
>    armed on a table with no hand.
> 3. The other player calls `sit_out()`. Fewer than two seats will now be dealt in, so
>    `advance_table_clock` stops short-circuiting on auto-deal and reaches the timer.
> 4. 31 seconds pass. Any tick — the on-chain clock, or any player's `check_timeouts` — reaches
>    `resolve_expired_action_timer`, which folds that seat out of the hand it has already been paid
>    for and calls `advance_game`.
> 5. `advance_game` had no phase guard. `count_active_players` is now 1, off last hand's cards, so
>    it calls `end_hand_single_winner` on the finished hand. `plan_payouts` "collects" 4,000,000 and
>    pays it out again.
>
> Measured, exactly, on the wasm the harness had just built:
>
> ```text
> BEFORE: ledger=1200000000 escrow=800000000 chips=400000000 pot=0 internal=1200000000
> AFTER : ledger=1200000000 escrow=800000000 chips=404000000 pot=0 internal=1204000000
> DELTA internal = 4000000
> CRITICAL: pot accounting disagreement in hand 1: state.pot = 0 but the contributions
>   (including 0 departed stake(s)) sum to 4000000. Settling from the contributions,
>   which is what was taken from the stacks.
> ```
>
> That `CRITICAL:` line, and a SECOND `HISTORY: hand 1` record for a hand that had already been
> archived, are the identical pair of lines the 400-step two-seed run produces at its step 233. The
> canister said what was wrong, once, and carried on.
>
> ### The fix: four guards, at four depths, three of them backstops
>
> | where | what it now does | its gate |
> |---|---|---|
> | `use_time_bank` | refuses when no hand is in progress, **before** the time bank is spent | `reg39_a_settled_hand_is_never_settled_a_second_time` |
> | `resolve_expired_action_timer` | a clock belonging to no hand is dropped, and folds nobody | `payout_tests::e41_an_expired_clock_on_a_finished_hand_resolves_to_nothing` |
> | `advance_game` | hard return when `!hand_in_progress` (this was a narrow guard on `refresh_side_pots` only) | `payout_tests::e41_advance_game_does_nothing_to_a_table_with_no_hand` |
> | `end_hand_single_winner` / `determine_winners` | `settled_twice_refusal`: refuse, and log `CRITICAL:` so the classifier convicts anything that ever gets here | `payout_tests::e41_a_settled_hand_refuses_to_settle_again` |
>
> Each of the last three gates was verified by removing **its own guard alone**: it goes red and
> the other two stay green. `reg39_*` was verified on the fully unfixed build in both of its legs —
> the door (`use_time_bank -> Ok(0)`) and the money (`created 4000000 e8s of chips out of nothing`).
>
> ### What was NOT done, on purpose
>
> `finish_hand` still leaves `total_bet_this_hand` standing. Clearing it there is the tempting
> one-line fix and it is wrong: the fuzzer's M3 basis (`wagered_last_hand`), the hand-attribution
> watch and the settlement oracle all read that figure at `HandComplete`, and it is the only record
> of what the hand collected. The defect is not that the record exists, it is that the settlement
> path was willing to treat a record as money.
>
> ### And the original reproducer
>
> ```text
> MONEY_FUZZ_SEEDS=212967420072193,212967420072194 MONEY_FUZZ_STEPS=400 MONEY_FUZZ_SHRINK=0
>   before: seed 0xc1b1576c1102 -> 5 hands, 7 blocking finding(s), worst stranded 3175980000 e8s
>   after : seed 0xc1b1576c1102 -> 5 hands, 0 blocking finding(s), worst stranded 0 e8s
> ```
>
> ### AND THE DEFAULT TARGET NOW REACHES IT, WHICH IS THE HALF THAT KEPT IT ALIVE
>
> The entry blamed "no default target runs the configuration". That was true and it was not the
> reason. **The fuzzer's ALPHABET could not express the call**: `Op::UseTimeBank` names a random
> actor and `use_time_bank` refuses anybody who is not `action_on`, so it essentially never landed;
> and the harness's only seat-resolver refused to look at the table between hands, which is the only
> place this defect lives. See [H-26](#h-26) for the two changes and the four-run measurement table.
>
> With them, the DEFAULT configuration — 3 seeds x 220 steps, nothing in the environment, the exact
> thing `./scripts/dev.sh test` and `make fuzz-default` run — finds it on the unfixed canister:
>
> ```text
> seed 0xc1ea2dec0002 -> 10 blocking finding(s), worst stranded 1249020002 e8s
> seed 0xc1ea2dec0003 -> 21 blocking finding(s), worst stranded 1459990000 e8s
> M1_CONSERVATION:ledger_equals_owed  delta=-200000000  (chips CREATED)
> M1_CONSERVATION:ledger_equals_owed  delta=-399000000  (chips CREATED)
> M1_CONSERVATION:ledger_equals_owed  delta=-601500000  (chips CREATED)
> CRITICAL: pot accounting disagreement in hand 2: state.pot = 0 but the contributions
>   (including 0 departed stake(s)) sum to 399000000.
> ```
>
> **2.00, 3.99 and 6.01 ICP, from the seed set the project has shipped since wave 1** — 50x to 150x
> the 4,000,000 e8s this entry is titled with. On the fixed canister the same run drops from 32
> findings to 2, and both survivors are the [E-89](#e-89) / [FINDING 31](SECURITY-FINDINGS.md#finding-31)
> dead-band family that was there before and is not this defect.

**The record of the defect as it was filed, kept because the entry was wrong about the cause for
nine waves and that is worth reading:**

The strongest thing the money-safety harness can say is M2 LEDGER REALITY: the canister's real
ledger balance is at least what it owes players. It can never be short. It goes short.

```text
cd tests/money_safety
MONEY_FUZZ_SEEDS=212967420072193,212967420072194 MONEY_FUZZ_STEPS=400 MONEY_FUZZ_SHRINK=0 \
  cargo test --test fuzz -- --nocapture
```

```text
M2_LEDGER_REALITY:canister_is_short|FundCreation|HandComplete|-   first observed at step 258, 143x
canister is SHORT: ledger_main=6800030001 but it owes
  escrow=6606030001 + chips=196000000 + pot=0 = 6802030001
M1_CONSERVATION:ledger_equals_owed|FundCreation|HandComplete|-    same step, same 143 observations
delta=-2000000 (chips CREATED: the canister owes money it does not hold)
```

2,000,000 e8s is exactly the big blind of the 6-max ICP table the seed runs. The shortfall appears
at a `check_timeouts` and never closes: from step 258 to the end of the run the canister owes 0.02
ICP it does not hold. Nobody has to withdraw it for this to matter — it is the accounting state in
which the LAST player to withdraw is the one who finds the money missing.

**Not caused by anything in this wave.** Reproduced on `$SCRATCH/pristine`, a `git archive HEAD`
extraction of commit `3253b67` with the wave-3 harness, no local modifications: same two findings,
same 2,000,000 e8s, same step. It is reported here rather than fixed because fixing it means
editing `src/table_canister/src/lib.rs`, which the wave-4 attribution task does not own.

> **RE-DRIVEN 2026-08-06 (wave-11 register pass). STILL OPEN, AND TWICE THE SIZE.** Same command,
> against the wasm `./scripts/dev.sh test` had just built and reported green on
> (`sha256 c05fbdc6e7a46b80d4ace7591819429e09ad32650c5763ed5e904a8906360a45`):
>
> ```
> seed 0xc1b1576c1101 finished: 6 hands, 9 upgrades, 0 blocking finding(s)
> seed 0xc1b1576c1102 finished: 5 hands, 5 upgrades, 7 blocking finding(s)
>
> canister is SHORT: it holds ledger_main=6200030000 + deposit_subaccounts=0 = 6200030000
>   but it owes escrow=5805030000 + chips=399000000 + pot=0 = 6204030000
> delta=-4000000 (chips CREATED: the canister owes money it does not hold)
> test result: FAILED.  93.41 s
> ```
>
> It is **4,000,000 e8s**, not the 2,000,000 recorded below, so the "exactly one big blind"
> reading is wrong or was always a coincidence. A leg that did not exist in wave 4 also fires:
> `refresh_solvency_is_not_available_to_an_ordinary_caller` — the canister is short and an
> ordinary player is rate-limited out of taking the reading that would reveal it.
>
> **The write-up this entry asked for now exists:
> [SECURITY-FINDINGS.md FINDING 39](SECURITY-FINDINGS.md#finding-39).** It also records why seven
> waves passed without anybody noticing: no target runs this configuration. `dev.sh test` fuzzes
> 1 seed x 40 steps, `make fuzz-default` 3 seeds x 220, and the reproducer is 2 seeds x 400.
> `make fuzz` runs seed `…194` at 600 steps and is therefore very likely red as well.

**Not yet root-caused, and not yet minimal.** It reproduces only in the two-seed form
([H-26](#h-26)), which also means the shrinker cannot be trusted to reduce it. The next step is a
hand-written reproducer: seat a player, let a `check_timeouts` run a hand out while a seat is
Disconnected ([E-32](#e-32)) or being vacated, and watch `ledger_main` against
`escrow + chips + pot` across the settlement. It should be recorded in
`docs/SECURITY-FINDINGS.md` as soon as the mechanism is known, because a canister that owes more
than it holds is one withdrawal away from a player's funds being unbacked.

> **THAT NEXT STEP WAS NEVER THE RIGHT ONE, AND IT IS WORTH SAYING WHY.** The entry told nine
> waves of readers to look at a hand being run out while a seat was Disconnected or being vacated,
> because `check_timeouts` was where the shortfall was first OBSERVED. Nothing was wrong with any
> of that machinery. The hand the money came out of had already finished; the timer was resolving a
> clock that belonged to no hand at all. **The reported site was the observer, not the cause**, and
> the entry's own suggested experiment would have watched the settlement of a LIVE hand, which is
> conserving and always was. What found it was asking the different question — why does the engine
> log `state.pot = 0 but the contributions sum to 4000000`, and what has to be true of a table for
> `pot` and `total_bet_this_hand` to disagree in that direction.

<a id="h-29"></a>
### H-29 — medium — the sha256 the harnesses print identifies a BUILD, not the code — STATUS: OPEN

> ### WAVE-11 RECONCILIATION: reproduced, localised, and bounded
>
> Measured 2026-08-06 by building the same source at `/p1` and at
> `/a/deliberately/much/longer/absolute/path/for/the/very/same/source`:
>
> | build | table_canister | lobby_canister | history_canister |
> |---|---|---|---|
> | bare `cargo build --release` | **MISMATCH** | identical | identical |
> | `RUSTFLAGS="-Cstrip=symbols --remap-path-prefix=…"` (what `recipes/rust-reproducible.hbs` sets) | identical | identical | identical |
>
> The mismatching pair differ in **exactly one section**, and it is the debug `name` custom
> section (319,927 bytes vs 319,946). Every semantic section is byte-equal, walked section by
> section: types, imports, functions, tables, memory, globals, exports, elements, the
> 2,315,929-byte code section and the 188,259-byte data section.
>
> So the scope of this entry is now precise: **the deliverable artifact IS a function of the
> source; the raw `cargo build` output is not.** The harnesses print the sha256 of
> `target/wasm32-unknown-unknown/release/table_canister.wasm`, which is the raw output, so
> "the wasm under test" is still a build identity rather than a code identity — that half is
> unchanged and is why this stays OPEN. What is no longer true is any suggestion that the
> DEPLOYED module hash wanders: the two locks in the recipe (`-Cstrip=symbols`, then
> `ic-wasm shrink` without `--keep-name-section`) both close it.
>
> Docker was unavailable, so `./scripts/verify-build.sh --two-paths` — the container check
> that also pins the toolchain — was NOT run. This measurement is path-independence on one
> machine, not cross-machine reproducibility.


[H-21](#h-21) is fixed: the compiler is pinned and the output directory can no longer be moved out
from under the hash. Fixing it surfaced a second, independent reason the same source can produce
two module hashes, and this one the pin cannot touch.

Measured. Two trees, `diff -rq` clean over `src/`, same `Cargo.lock`, same `rust-toolchain.toml`,
both built `RUSTUP_TOOLCHAIN=1.90.0 cargo build -p table_canister --target wasm32-unknown-unknown
--release`:

```text
/Users/josh/Desktop/cleardeck   5996594741afd63f88f38098c5461a24fbacafbda16278fc44bd3317a6c55c23   2327409 bytes
$SCRATCH/plant                  3b847559f9e705e6e4102dff2d47ffddfd9587c96d81a56641380629b10d4b4a   2327468 bytes
```

Each is STABLE: `touch src/table_canister/src/lib.rs && cargo build` three times in a row gives the
same hash in each tree. And earlier in the same session the second tree produced
`5996594741afd63f…`, the same hash as the first; it stopped after that tree was once built with a
different toolchain, which is a build-cache state, not a source change.

Where they differ, exactly:

```text
first differing byte: 2053251 of 2327409  -- inside the `name` custom section's length prefix
  A  ... 0x85 0xdc 0x10  "name" ...        payload 1096325 bytes
  B  ... 0xc0 0xdc 0x10  "name" ...        payload 1096384 bytes   (+59)
```

Every byte before that — the type, import, function, table, memory, global, export, element, code
and data sections, i.e. everything the replica executes — is IDENTICAL. The whole difference is 59
bytes inside a 1.09 MB table of mangled symbol names that carries no semantics.

**Why it matters anyway.** The identity story this project tells is "every result is attributed to a
sha256, and the replica's `module_hash` is asserted to equal it". That is still true and still
worth having. What is NOT true is the inference a reader will make from it: that two runs printing
different hashes were testing different code, or that two runs printing the same hash is what
proves they were testing the same code. A reviewer comparing a wave-3 number against a wave-4
number by hash alone can be told the engine changed when only a symbol table did.

**Fix, cheap.** Hash the module with its `name` section stripped — or build with
`-C strip=symbols` / `--remap-path-prefix` and hash the result — and print BOTH: the deployable
module hash (what `canister_status` reports, which must keep matching) and a code-only digest that
is a fingerprint of the source. Deliberately not done here: `announce()`'s output and the
`module_hash` assertion are load-bearing for H-01, and changing what "the sha256" means is a change
every document in `docs/` quotes.

<a id="h-28"></a>
### H-28 — high — the tolerated-self-report list and the register did not meet, so the fuzzer's own default was red — STATUS: FIXED (wave 3)

**Reproduced first.** `cd tests/money_safety && cargo test --test fuzz`, with no environment at
all, failed at `fe72d46` in 221 s:

```text
money-fuzz: seed 0xc1ea2dec0003 finished: 5 hands, 3 upgrades, 1 blocking finding(s)
the fuzzer found 1 invariant violation(s) that are NOT documented defects.
  M1b_POT_BREAKDOWN:canister_reports_its_own_inconsistency|BreakdownDrift|HandComplete|+
  (seed 0xc1ea2dec0003, 12 ops) -- the canister logged a self-report that is on the
  tolerated list and settled anyway: WARNING: seat 2 carries both a live stake and a
  departed stake in hand 1 (docs/DEFECTS.md E-36)
```

Read the two halves together:

* `check_self_reported_inconsistency` matches the line against
  [`TOLERATED_SELF_REPORTS`](../tests/money_safety/src/documented.rs), and because it matches,
  downgrades it from `SelfReportedFailure` to `BreakdownDrift` on the check
  `canister_reports_its_own_inconsistency`. The comment says the accepted ones are "named in
  `TOLERATED_SELF_REPORTS` where they can be counted".
* `classify` then asks `REGISTER` whether anything names
  `(M1bPotBreakdown, "canister_reports_its_own_inconsistency")`. `REGISTER` is **empty on
  purpose** — "an empty register means nothing on the money path is excused" — so the answer is
  `Blocking(NotRegistered)` and the run fails.

So the downgrade buys nothing: a line on the tolerated list blocks exactly as hard as a line that
is not. The H-20 fix moved E-36's `WARNING:` out of a string the classifier could not read and into
one it can, and stopped one step short of the register entry that would let the fuzzer explore past
a defect the project has already decided to ship.

**Not caused by the wave-4 attribution work.** Reproduced identically on `$SCRATCH/pristine`, a
`git archive HEAD` extraction of `3253b67` with the pristine harness: same seed, same signature,
same 12-op shrunk reproducer. `make test` and `make fuzz` both pass explicit `MONEY_FUZZ_SEEDS`, so
neither of them lands on seed `0xc1ea2dec0003` and neither has ever shown it.

**Is it a real engine defect or an over-strict invariant? Neither.** The shrunk 12-op reproducer
answers it, and it is worth reading in full, because the ops ARE E-36:

```text
seed 0xc1ea2dec0003, 6-max ICP table, ante 500_000, 12 ops
 1. FundEscrow  { actor: 0, amount: 800_000_000 }
 2. JoinTable   { actor: 0, seat: 0 }
 3. FundEscrow  { actor: 1, amount: 800_000_000 }
 4. JoinTable   { actor: 1, seat: 1 }
 5. FundEscrow  { actor: 2, amount: 800_000_000 }
 6. JoinTable   { actor: 2, seat: 2 }
 7. StartNewHand{ actor: 0 }
 8. LeaveTable  { actor: 2 }          <- seat 2 vacated MID-HAND, stake stays in the basis
 9. ActInTurn   { act: AllIn }
10. FundAndSeat { actor: 2 }          <- the same chair re-occupied mid-hand
11. ActLegalInTurn
12. ActAs       { actor: 2, act: Call }  <- the newcomer BETS into a hand it holds no cards in

violation: M1bPotBreakdown / canister_reports_its_own_inconsistency
severity : BreakdownDrift    delta_e8s: 0    phase: HandComplete
detail   : the canister logged a self-report that is on the tolerated list and settled
           anyway: WARNING: seat 2 carries both a live stake and a departed stake in hand 1
```

Ops 8, 10 and 12 are the [E-36](#e-36) sequence verbatim, and E-36 is already documented, already
open, already shipping. The engine's *handling* of the state is correct — both stakes stay in the
payout basis under their own owners, because dropping either destroys a chip — so the `WARNING:` is
not an accounting inconsistency and the invariant that notices it is not over-strict either. It is
right to notice. What was broken is that the harness had nowhere to put the noticing: the downgrade
to `BreakdownDrift` on `canister_reports_its_own_inconsistency` bought nothing, because no register
entry named that pair. `worst_delta` is `0` on all 8 occurrences: not one e8 moved.

**So no engine change, and the engine stays byte-stable.** `table_wasm_sha256` in the report before
and after this pass is the same module.

**Fixed by naming the tolerance, which was the decision, not the typo.** A
`DocumentedDefect { id: "E-36", invariant: M1bPotBreakdown, check:
"canister_reports_its_own_inconsistency", directions: &[Unsigned], max_abs_delta_e8s: Some(0) }` is
now in `REGISTER`. The other option — stop emitting a violation for a tolerated line — was
rejected: it puts tolerance back into a place the register cannot see, which is the exact H-03/H-20
pathology `documented.rs` exists to prevent (296 occurrences of a real open defect once reported as
`0 documented finding(s)`). The bound is **zero e8s** and the direction is `Unsigned` only, so the
entry can excuse exactly one thing: a zero-delta log line on that one check. It cannot excuse a
chip. `register_entries_are_all_still_needed` now polices both halves — the entry's id must stay in
this file and the `TOLERATED_SELF_REPORTS` substring must stay in `lib.rs` — so fixing E-36 forces
both to be deleted together. **That coupling is E-36's known-defect marker**, and it lives in the
harness rather than in `DEFECT_MARKERS` because E-36 is not reachable from
`tools/differential`'s pure-engine subset.

After the fix, the same bare invocation:

```text
money-fuzz: seed 0xc1ea2dec0003 finished: 5 hands, 3 upgrades, 0 blocking finding(s),
            1 documented finding(s), worst stranded 0 e8s
  documented  M1b_POT_BREAKDOWN:canister_reports_its_own_inconsistency|BreakdownDrift|
              HandComplete|+ x8 worst_delta=0
money-fuzz: M8 PRINCIPAL ATTRIBUTION ran on 13 of the 15 hand(s) this run completed;
            10 of them were also checked against the independent settlement oracle
test result: ok. 1 passed; 0 failed  (51.02s)
```

The defect is now COUNTED and PRINTED instead of either blocking or vanishing, which is the whole
point of the register.

**And the invocation is wired in, because a red default nobody runs is the actual defect.** The
reason this survived a whole wave is structural, not careless: every caller passed
`MONEY_FUZZ_SEEDS`, so `seeds()`'s `Err(_) =>` arm and `DEFAULT_STEPS` were executed by NOTHING.
`make test` passed 1 seed × 40 steps, `make fuzz` passed 9 explicit seeds, and neither ever landed
on `0xc1ea2dec0003`. Three changes, so it cannot recur:

1. **`make fuzz-default`** / `./scripts/dev.sh fuzz-default` — the bare invocation, run under
   `env -u MONEY_FUZZ_SEEDS -u MONEY_FUZZ_STEPS -u MONEY_FUZZ_SHRINK -u MONEY_FUZZ_REPORT` so a
   shell that happens to export one of them cannot quietly turn it back into a different run.
2. **`make fuzz` runs it FIRST**, before the 9-seed sweep, so the long gate can never be green
   while the command in the harness's own doc comment is red.
3. **`make test` runs it too**, unconditionally and with no opt-out, after the existing steerable
   40-step smoke. It costs ~51 s against a gate that already spends 900 s on the settlement
   oracle, and it reuses the wasm and the ledger that step has already fetched. An environment
   variable that skips it is how the hole gets dug a second time, so there is not one.

<a id="h-27"></a>
### H-27 — medium — what the widened attribution gate still cannot reach — STATUS: OPEN

Wave 4 made principal attribution a property of every settled hand rather than of two scripted
fixtures, and earned it against six planted misdirection bugs (the matrix is in
[the attribution section](#attribution-what-was-planted-and-what-convicted-it)). All six are
convicted. Two of them are convicted by exactly ONE of the three suites, and that is the shape of
hole this entry exists to name.

| planted bug | reached by | NOT reached by | why |
|---|---|---|---|
| two winners' amounts swapped | the settlement oracle's ladder scenarios | the hand-written M8 fixtures, the fuzzer at 3 seeds x 300 steps | the swap needs **two payouts to two different people in one hand**: a chopped pot, a side-pot ladder with different winners per layer, or a multi-way refund. The ordinary-hand fixtures are four-handed hands with one winner, and the fuzzer produced no chop in 15 hands |
| `push_winner` merges two principals | **nothing, as of 2026-08-05** | everything | the merge needs **one chair carrying two owners' stakes** (E-36), and E-36 is FIXED: a mid-hand arrival is never dealt the action, so it can never stake a hand, so no chair can carry two live stakes. The test that used to reach this is now `a_mid_hand_arrival_is_never_dealt_the_action_and_can_never_stake_the_hand`, which asserts the impossibility instead. `push_winner`'s aggregation by `(seat, principal)` is retained as defence in depth and is now UNREACHED by any suite -- that is a real coverage loss and it is named here rather than hidden |

Neither gap is closed by running longer; both are shapes the generators do not aim at. The fixes
are cheap and specific:

1. give the money-safety harness a chopped-pot fixture — it cannot search for deals the way
   `tests/settlement` can, so the cheapest version is a heads-up hand where the board plays and
   both players chop;
2. give `tests/settlement` a chair-swap scenario in which the newcomer BETS, so its record leg has
   two owners at one seat to tell apart;
3. weight the fuzzer's generator towards chops and towards `sit_in`-after-`join_table`-mid-hand.

Also still true: the oracle leg declines a hand whose chair carried two owners (the rules of poker
do not split a per-seat answer between two people), and the whole gate declines a hand it could not
fully observe. Both are counted and printed per run — `M8 ATTRIBUTION: 6 hand(s) finished, 5
measured by principal, 4 of those also checked against the independent settlement oracle` — because
an instrument that quietly declines is indistinguishable from one that passes.

<a id="e-30-correction"></a>
### E-30 — correction — the incomplete-all-in rule as first implemented was not the rule — STATUS: FIXED (wave 2)

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
### E-31 — correction — resolving the stale timer before the whose-turn check let a wager cross streets — STATUS: FIXED (wave 2)

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
### H-22 — medium — the settlement oracle's exact-deal search is unbounded, so a degenerate deck HANGS it — STATUS: OPEN

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
### H-23 — high — CI runs none of the fund-safety harnesses — STATUS: FIXED (wave 13)

`.github/workflows/ci.yml` had five jobs: build the canisters for wasm32, `cargo test --locked
--workspace`, the `poker_core` wasm32 golden replay (plus the outsider verifiers), Candid interface
drift, and the frontend build. That is a genuinely good set — the wasm32 job in particular is what
would have caught FINDING-02 — but note what was absent:

* the money-safety suite (`invariants`, `regressions`, `deposit_replay`, `fuzz`),
* the settlement oracle,
* the custody gate (`controller_custody`), and
* the solvency gate.

**Every fund-safety claim in `WAVE-02.md` and `SECURITY-FINDINGS.md` came from a harness that no
CI job invoked.** They ran when a human remembered to run them.

**`--workspace` could not have reached them even in principle.** `tests/money_safety`,
`tests/settlement` and `tests/no_peeking` each carry a bare `[workspace]` table, and that is
deliberate and correct: `pocket-ic` drags in tokio + reqwest + rustls, and the root `Cargo.lock` is
an input to `cargo build --locked` for six canisters that custody real ICP on mainnet. Detaching
them keeps the deployed modules byte-reproducible. It also put every money invariant outside the
only test command CI ran.

## What was built

| file | what it is |
|---|---|
| `scripts/ci-fund-safety.sh <fast\|deep>` | **the job body**, not a description of one. Both workflows call this and nothing else, so the thing a developer runs on a laptop and the thing that gates a merge are the same bytes |
| `scripts/test-suites.list` | **the one list**: every cargo test target in the repo, its tier, its thread count, its filter, its per-row environment, and one line saying why it is in the tier it is in. The runner executes these rows; they are not a description of what CI runs, they *are* what CI runs ([H-47](#h-47)'s lesson) |
| `scripts/pocket-ic.sh` | resolves a PocketIC **server**: `$POCKET_IC_BIN`, then this repo's cache, then the dfx cache, then a pinned download. The required version is **read out of `tests/money_safety/Cargo.lock`**, so a crate bump cannot silently leave CI on an older server — it lands on a version with no recorded checksum and the script stops |
| `scripts/check-suite-wiring.sh` | the unwired-suite gate, below |
| `.github/workflows/ci.yml` | two new jobs: `fund-safety-fast` (required, every PR) and `suite-wiring` |
| `.github/workflows/fund-safety-deep.yml` | the scheduled tier: nightly, on `workflow_dispatch`, and on every push to `main` |

The two fetches are both pinned and both verified on use. The PocketIC server is checked against a
recorded sha256 per platform *and* required to answer `--version` with the version the lockfile
asks for — measured, because the release asset extracted under any other filename **panics on
startup** (`The PocketIc server binary name must be "pocket-ic" or "pocket-ic-server"`), so the
version in the cache path is a directory rather than a suffix. The ICP ledger's URL and hash are
not restated in the shell at all: they are **read out of `tests/money_safety/src/wasms.rs`**, which
owns the pin, because a CI job that fetched a different ledger than the harness pins would
invalidate every M2 (LEDGER REALITY) result while staying green.

## The split, and why

`./scripts/dev.sh test` takes over twenty minutes. A required check that takes an hour gets marked
non-required, and a job nobody waits for gets skipped, so the split is by **what the runtime buys**:

**FAST (required on every PR — 21 suite invocations).** Everything that convicts a money defect in
seconds. Measured on a laptop, warm build cache, one suite at a time: `invariants` 63 s,
`deposit_replay` 26 s, `controller_custody` 23 s, `admin_custody` 22 s, `deposit_surface` 22 s,
`deposit_subaccount_anchor` 20 s, `solvency` 16 s, `ledger_boundary` 16 s, `oldest_cluster` 13 s,
`fund_reachability` 13 s, `regressions` 11 s, `wave6_coherence` 9 s, `deposit_floor` 8 s,
`coherence_w8` 5 s, `ui_limits` 0 s — about five minutes of tests. Plus a short fuzz run, and the
settlement oracle with its randomised sweep cut from 24 hands to 6.

**The settlement oracle is in the fast tier deliberately, however long it takes.** It derives what
each seat is owed from the rules of poker, independently of the code under test, and compares that
against what the canister actually paid — which is what convicts a payout with the right totals at
the wrong seat, the cross-agent signature this project has now produced eight times. (`invariants`
grew its own attribution oracle, M8, after [FINDING 13](SECURITY-FINDINGS.md#finding-13), and the
planted-theft run below shows both convicting. Two independent instruments on the property that has
cost this project the most is the right number, not a redundancy to trim.)

**Measured end to end, green: 21 rows, 222 tests, 1,491 s.** That figure is the pessimistic one —
a fresh clone with a cold build cache (so every test binary is compiled inside its own row) and
three other test runs competing for the same laptop. `controller_custody` reads 98 s there against
23 s warm, `regressions` 74 s against 11 s. With the build cache warm the tier is about seven
minutes of tests. `timeout-minutes` on the job is set for the cold case, because a required check
that goes red for a cache miss is a required check somebody removes.

**DEEP (nightly, on dispatch, and on every push to `main`).** The runs whose value is depth:

| suite | measured | why it cannot be a required check |
|---|---|---|
| `stall_agreement` | **802 s** | 138 forked worlds, one per row ([E-59](#e-59)). On its own it is two thirds of the twenty-minute local gate |
| `fuzz` (9 seeds × 600 steps) | minutes | [E-39](#e-39) was found at exactly these settings and by nothing shorter; the wave that ran a third of it reported "0 blocking findings" |
| `fuzz` (no environment at all) | ~50 s | [H-28](#h-28): every caller passed `MONEY_FUZZ_SEEDS`, so the harness's own default arm was executed by nothing and was RED for a whole wave |
| `timers` | slow by construction | it *waits*: every test drives the table with no ingress message after setup |
| `cycles_runway` | slow by construction | burn measured over many hands ([E-55](#e-55)) |
| `settlement` (24-hand sweep) | minutes | the shape nobody thought of |

Pushing to `main` runs the deep tier too, so a merge that breaks a deep suite is attributed to that
merge rather than found by a nightly the next morning with a day of commits to bisect.

**The deep tier was RED on its first run, and both reds are true.** Row 1 — the fuzzer at its own
defaults, 665 s — reported two blocking findings against the working tree:

```
M9_FUND_REACHABILITY:money_left_behind_after_drain|FundsUnreachable|HandComplete|+
  (seed 0xc1ea2dec0002, 5 ops) -- after every legal player-side exit was driven to
  exhaustion, the canister still owes 20002 of the 3793874570 e8s it started with,
  and no player call can move it.

M3_NO_RAKE:value_conserved_across_hand|FundCreation|HandComplete|+
  (seed 0xc1ea2dec0001, 7 ops) -- value at the table changed across a hand with no
  external money movement: before total 2400000000, after total 2400010000, delta=10000
```

The first is [E-89](#e-89) exactly, which is OPEN and whose own entry says the red is true. The
second is a **fund CREATION** — 10,000 e8s that nobody put in — which `src/fuzz.rs`'s own classifier
treats as never excusable.

**Re-running it turned up something worse than either.** The same invocation was run again on a
separate checkout, and the two runs disagree with an IDENTICAL module:

| | repo root (three other test runs on the machine) | clean twin (quiet) |
|---|---|---|
| module sha256 | `713d5678…798b7` | `713d5678…798b7` (same bytes) |
| seed `0xc1ea2dec0001` | 2 hands, 4 upgrades, **1 blocking: FundCreation +10,000 e8s** | 3 hands, 5 upgrades, **0 blocking** |
| seed `0xc1ea2dec0002` | 2 hands, 0 upgrades, 1 blocking, 20,002 stranded | 7 hands, 3 upgrades, 1 blocking, 20,002 stranded |

Same wasm, same default seeds, same `DEFAULT_STEPS = 220`, `env -u` on every `MONEY_FUZZ_*` in both.
**The fuzzer's default run is not reproducible run to run**, and the difference tracks machine load,
so something outside `(seed, config, actor_names, steps)` is steering it. That is [H-26](#h-26) at a
larger scale than H-26 describes — H-26 is about seed ORDER inside one process; this is one
invocation against one module giving two different op sequences — and it has two consequences
worth writing down:

1. The `FundCreation` red is **unattributed**. It was produced once, by the shipped module, and did
   not reproduce on a quieter machine. It is not dismissed on that basis; a fund-creation finding
   that appears under load is a fund-creation finding.
2. **The deep tier's two fuzz rows will be intermittently red**, and a nightly that flickers is a
   nightly people stop reading. The fix is [H-26](#h-26)'s, not this entry's: make a run a function
   of its inputs. Until then the fuzz rows are honest but noisy, which is still strictly better than
   a fuzzer nothing schedules.

## A suite that nothing runs now fails the build

This is the fifth filing of one defect: [H-17](#h-17) (`deposit_replay`, the only proven
fund-theft reproducer in the project, named by nothing for a wave), [H-45](#h-45) (six suites,
46 tests, one of them RED the whole time while the register cited it as a gate), [H-50](#h-50)
(`tools/archive`, 39 self-tests), [H-53](#h-53) (`tests/no_peeking`, five targets named in their
own `Cargo.toml` *so that nothing would be auto-discovered*, then named by nothing else). Every one
was found by a human reading a directory listing.

`scripts/check-suite-wiring.sh` is that reading, mechanised, and it is a required CI job. It
discovers every cargo test target from every `Cargo.toml` in the tree — not from the inventory,
because a checker that only looks where it has been told to look cannot see a new crate — and
fails when a target has no row, when a row has no target, when a tier has no runner, or when
`dev.sh` names a `--test` target CI has never heard of.

**It caught three while it was being written**, which is the whole argument for having it:

1. `cycles_runway.rs` and `e41_probe.rs` were dropped into `tests/money_safety/tests/` by another
   owner *during this wave* and were named by nothing — no `[[test]]` stanza, no `dev.sh` line, no
   make rule, no CI job. The check said so within a minute.
2. `e41_probe.rs` was then deleted by its owner while the inventory was being written, and the
   check went red on the row that outlived the file. That is the other direction, unrehearsed.
3. **Its own blind spot.** The first version looked only at integration targets. `cargo test
   --test X` does not run `--lib`, and every runner in this repository names integration targets,
   so unit tests compiled into a crate were invisible to the gate whose entire job is finding
   invisible tests. Extending discovery to `#[cfg(test)]` inside `src/` found **twelve tests that
   nothing ran**: one in `tests/money_safety/src/invariants/outcome.rs` on `capped()`, the
   arithmetic that decides how much of a bet is *contestable*, inside the money-safety harness
   itself; and eleven in `src/no_peeking/dealer_types` + `dealer_canister`, the spike whose five
   integration targets were [H-53](#h-53). All twelve are wired now — the money-safety one into the
   fast tier, the spike's into `dev.sh no-peeking`.

`--selftest` plants every failure the check claims to catch — a new unwired test file, a whole new
crate whose only tests are unit tests, a row whose file is gone, a tier no workflow runs, the
settlement oracle demoted out of the required tier, and a tier name nothing recognises — and
requires the check to go red on each, with a green baseline either side so the reds are not
vacuous. **The CI job runs `--selftest` before it runs the check**, because this repository has
already shipped a gate that could not fail: the Candid drift job compared the wrong thing and ended
in `exit 0` behind a TODO while the committed `.did` was missing seven fields and a whole method.

**The tier assignment is held against the pressure that will be put on it.** The fast tier is the
one that costs every pull request, so the incentive on it is always downward, and moving the
custody gate or the oracle to a nightly is a one-word edit that leaves every other check green.
`REQUIRED_FAST` in `check-suite-wiring.sh` names the nine gates that have convicted a real fund
defect here, and the selftest performs the one-word demotion of the settlement oracle and requires
the build to break.

## The runner cannot pass by testing nothing

`cargo test` exits 0 when a filter matches nothing, so a renamed filter would turn a gate into a
silent no-op — this project's most reliable failure shape. Each row is therefore required to have
produced a `test result:` line, a row with a filter is required not to report `0 passed`, and a
tier that selects no rows at all stops the run instead of passing it. All three were executed
against the real script and a real crate:

| planted | what the runner did |
|---|---|
| a filter matching no test | `FILTER MATCHED NOTHING — "a_filter_that_matches_no_test_at_all" selects no test in oracle_rules`, exit 1 |
| a row naming a target that is not there | `NO TEST BINARY REPORTED — this row executed nothing`, exit 1 |
| a tier with no rows | `ERROR no rows in scripts/test-suites.list for tier 'fast'`, exit 1 |
| *control:* one genuine row | `the fast fund-safety tier is green`, exit 0 |

The control is the point: without it the three reds would only prove the script refuses everything.

## The pipeline was made to go red on a real theft

A green pipeline that cannot fail is the thing this project keeps finding, so the tier was
executed against a planted fund-theft rather than argued about. In a scratch clone,
`apply_payouts` in `src/table_canister/src/lib.rs` was changed to divert **1% of every pot** into
an escrow account the operator controls, while the winner record continued to state the full
amount:

```rust
let skim = payout.amount / 100;
if let Some(ref mut p) = state.players[payout.seat as usize] {
    p.chips = p.chips.saturating_add(payout.amount - skim);
}
if skim > 0 {
    credit_escrow(Principal::management_canister(), skim);
}
```

Chosen to be **invisible to every conservation check in the canister**: the payout plan still
conserves, `sum(winners) == collected` still holds (M3 NO RAKE), and `total_liability()` is
unchanged because the chips leave a stack and arrive in an escrow, which is also a liability.
Correct totals, wrong recipient, every invariant silent — the signature. The plant lives only in a
scratch clone; nothing in this tree contains it, and the two module hashes differ
(`713d5678…` clean, `5dd6a6ab…` planted), so neither run can have been against the wrong wasm.

**The same tier, on the two modules:**

| | clean `713d5678…` | planted `5dd6a6ab…` |
|---|---|---|
| result | **green**, exit 0, 21 rows, 222 tests, 1,491 s | **RED**, exit 1, 8 of 21 rows, 1,764 s |
| rows that convicted | — | `invariants`, `admin_custody`, `oldest_cluster`, `wave6_coherence`, `fund_reachability`, `fuzz` (1 seed × 40 steps), `disagreements -- pinned`, `settlement` |
| rows that stayed green under the theft | — | `regressions`, `deposit_replay`, `ui_limits`, `deposit_floor`, `ledger_boundary`, `coherence_w8`, `deposit_surface`, `deposit_subaccount_anchor`, **`solvency`**, `controller_custody`, `(lib)`, `oracle_rules`, `disagreements -- golden` |

The second row of that table is the point of the design and the third row is the reason the fast
tier is the shape it is. **`solvency` stayed green while 1% of every pot was being stolen** — it
must, because the money is all still inside the canister and the books still balance. So does
`regressions`. What convicts is attribution, and there are two independent instruments on it:

```
M8_PRINCIPAL_ATTRIBUTION violated:
  hand #1: principal aaaaa-aa staked NOTHING and came out of it +640000 e8s richer.
  You cannot win money from a hand you did not play.
```

```
`post_flop_betting_to_showdown` was settled differently from the rules of poker.
Per-SEAT DIFF   (engine minus what the rules owe): [(0, -2), (1, 0), (2, 0), (3, 0)]
Per-PRINCIPAL DIFF (WHO was paid, by principal):   [("aaaaa-aa", 2), ("6ui2b-…-oae", -2)]
Per-RECORD DIFF (who the canister SAYS it paid):   []
Every chip may still be conserved -- 0 destroyed -- and the totals may still be right;
what is wrong is WHO HAS THE MONEY.
```

`Per-RECORD DIFF []` is the whole argument: the canister's own record of the hand was **clean**,
because the plant credits the winner record in full and skims the stack. An instrument that reads
the canister's record agrees with the canister. Only an instrument that measures what actually
moved, against an answer derived somewhere else, can say otherwise.

## What this does not fix

* The deep tier's cost is real. A nightly that nobody reads is a nightly that fails silently;
  the workflow keeps every suite log and the fuzzer's machine-readable report as artifacts so a
  reader can say what it actually covered, but nothing forces anyone to look.
* [H-45](#h-45) is not closed here. Its three named-as-unwired targets (`stall_agreement`,
  `solvency`, `fund_reachability`) are now in `scripts/dev.sh`, in the inventory and in a CI tier,
  and the class it describes now fails the build — but the row is another owner's to close.
* The first CI run on a cold cache pays to build `pocket-ic` + tokio + rustls for two detached
  crates. `Swatinem/rust-cache` is configured for all three workspaces; the cost is the first run
  after a lockfile change, not every run.
* **Ten rows are gated by `scripts/dev.sh` and by no CI job**: `tests/no_peeking`'s five targets
  plus its lib, `tools/differential/fast_subset` plus its lib, and the two sealed-dealer crates.
  They are run by `dev.sh test` (steps 3 and 8) and nothing in CI types that. The check prints
  `LOCAL GATE ONLY, no CI job` on every one of them and the census ends `41 of 51 row(s) are gated
  by a CI job; 10 are local-only`, because a tier line that just read `ok` would let a reader
  conclude they run on every pull request. They do not. Wiring them is the obvious next step and
  was out of this pass's scope (`money_safety`, `settlement`, the custody gate).
* **`scripts/dev.sh test` and the fast tier are still two lists.** The check enforces the direction
  that matters — every `--test` target `dev.sh` names must have a row, so the local gate cannot run
  something CI has never heard of — and `REQUIRED_FAST` in `check-suite-wiring.sh` stops the gates
  that have convicted a fund defect from being demoted to the nightly. It does **not** stop
  `cmd_test` from dropping a target that CI still runs. The clean fix is for `dev.sh test` to call
  `./scripts/ci-fund-safety.sh fast`, which is a change to a file three other owners were editing
  during this wave.
* The deep tier was run once, here, and takes long enough that nobody will watch it. It keeps its
  logs and the fuzzer's report as artifacts; nothing forces a reader.

<a id="e-39"></a>
### E-39 — medium — `leave_table` reduced `state.pot` without rebuilding `state.side_pots` — STATUS: FIXED (wave 3)

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

> **HISTORICAL SNAPSHOT — the status column below is NOT maintained.** It is the queue as it was written in this wave. The only true statuses are in [THE REGISTER](#the-register).

| # | sev | status | where | one line |
|---|---|---|---|---|
| [T-10](#t-10) | **high** | **FIXED IN THIS PASS** | `PokerTable.svelte` `$effect` | `JSON.stringify` on a Candid `nat64` threw **19 uncaught TypeErrors in one ordinary hand**, killing the action log and starving the effects the fairness panel and hand history run on |
| [T-11](#t-11) | **high** | **FIXED IN THIS PASS, now gated** | `+page.svelte` `.current-table-name` | the largest string on every table screen quoted the LOBBY's stale blinds: `6-Max - 0.01/0.02` on a table charging 0.05/0.10 and `9-Max - 0.01/0.02` on one charging 0.10/0.20. 18 scenes were filed "agrees with chain: yes" around it |
| [E-40](#e-40) | **high** | open | `record_hand_to_history` (`lib.rs:871`) | the only unguarded `evaluate_hand` call left in the canister. Observed **trapping on the live local canister** during ordinary browser play: `IMPOSSIBLE HAND … got 0 community`. A trap here cannot settle the hand |
| [T-14](#t-14) | **high** | open | `tools/shots/lib/frontend-build.mjs` `buildEnvFor` | the deployed local frontend points its agent at **127.0.0.1:4943** while the gateway is on 8077, so the app only works behind the screenshot harness's own shim. Opened in a plain browser it shows a raw fetch stack trace and "The lobby canister is reporting no tables" |
| [H-24](#h-24) | **high** | open | `tools/shots/scenarios/shuffleproof.mjs` | the fairness scene asserts `.proof-item >= 2`, which is true **before** the verification runs. Both shipped shuffleproof PNGs show rungs 3 and 4 grey and "Re-deriving your cards locally", filed as verified |
| [T-16](#t-16) | **high** | **FIXED IN WAVE 4** | the whole client at 390×844 | the mobile playing surface was **19.7–21.1% of the screen against PokerNow's 52.1% on the identical device**, and two of the six mobile captures did not contain a poker table at all. Now **42.9–52.1%**, scroll-anchored, nothing off-frame |
| [D-03](#d-03) | medium | **partly fixed** (vocabulary landed) | every component `<style>` block | 16 border radii, 28 font sizes, 9 greens, 6 ambers, 11 greys, 8 panel tints, 8 panel strokes; four buttons in one header row with three heights, two radii, two font sizes and two accent families |
| [T-19](#t-19) | medium | **FIXED IN WAVE 5** | `src/cleardeck_frontend/src/app.html` + `+page.svelte` `.header-right` | every phone renders the whole app at **0.918 scale**: `.header-right` needs 424 CSS px, the viewport meta has no `initial-scale`, so Chrome zooms the document out to fit. Every glyph is 8.2% smaller than authored and ~32 px of the screen's right edge is blank. **Fixed**: `initial-scale=1` plus a two-row table header whose `.header-right` is 374 px. Measured after: layout viewport `390x844`, `documentElement.scrollWidth = 390`, page scale `1.0000`, at 320/360/390/430/768 px wide |
| [T-15](#t-15) | medium | **partly fixed** (first consumer) | `lib/utils.js` `formatTokenAmount` | the "canonical money layer" written this wave, documented at length, was imported by **nobody**. Seven copies of "divide by 1e8", not one |
| [T-12](#t-12) | low | **FIXED IN THIS PASS** | `PokerTable.svelte:769` | the MAIN pot was labelled `Side 1`, and on a single-layer pot it printed `SIDE 1 0.40` directly under `TOTAL POT 0.40` |
| [T-13](#t-13) | low | **FIXED IN THIS PASS** | 3 of 4 dialogs | Escape closed `HowItWorks` and silently did nothing in `DepositModal`, `WithdrawModal` and `HandHistory`; each carried a keydown handler on a `tabindex="-1"` backdrop that nothing can focus |
| [H-25](#h-25) | medium | open | `./scripts/dev.sh known-defects` | one marker, for one low-severity defect. Twelve open engine defects in this register have none |
| [T-18](#t-18) | **high** | **FIXED IN THIS PASS** | `WithdrawModal.svelte:74` | the withdrawal confirmation printed the ledger **block index** as an ICP amount, and never stated the fee. Measured: withdrawing 1 ICP at block 1130 said "0.0000 ICP sent to your wallet" |
| [T-17](#t-17) | low | **FIXED IN THIS PASS** | `HandHistory.svelte` download button | the per-hand JSON export threw on the same BigInt class as T-10; fixed here, but it was never on any screen the harness photographs |

<a id="t-10"></a>
### T-10 — high — one uncaught BigInt threw 19 times a hand and starved three features — STATUS: FIXED (wave 3)

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
### T-11 — high — the table header priced the table 5× and 10× wrong — STATUS: FIXED (wave 3)

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
### E-40 — high — the one unguarded `evaluate_hand` left, and it traps on the live canister — STATUS: FIXED (wave 5)

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
### T-14 — high — the app a human opens is not the app the gate tests — STATUS: FIXED (wave 14)

> **FIXED 2026-08-09, including the harness change this entry deferred to "wave 4".**
>
> `buildEnvFor` now sets `VITE_LOCAL_GATEWAY_PORT` and `VITE_LOCAL_HOST` from the harness's own
> `GATEWAY_PORT`, so the bundle deployed to the local asset canister points at the gateway this
> project actually runs. `run.mjs` and `perf.mjs` therefore no longer start the 4943 reverse proxy:
> they open `http://<frontend-id>.<host>:<port>/`, **the URL a human types**. The proxy survives
> only for `SHOTS_SERVE_DIST`, which deliberately serves a different build and is already labelled
> as such in the manifest.
>
> **Measured in Chrome, no shim, no harness:** `http://5uljf-s3777-77775-aaaea-cai.localhost:8077/`
> renders `3 tables · 0/17 seats · 0% rake`, all three rows with live stakes read from their
> contracts, and no console error. Before, the same URL showed a raw fetch stack trace and *"The
> lobby canister is reporting no tables."*
>
> **Gate:** `assertBundleLocalGateway(GATEWAY_PORT)`, called by `buildFrontend` next to the existing
> canister-id assertion — because that one was green through the whole of this defect. Built once
> with the PRE-FIX environment to prove it convicts:
>
> ```text
> ABORT: the built bundle in src/cleardeck_frontend/dist contains no reference to the local
> gateway port 8077. VITE_LOCAL_GATEWAY_PORT did not reach the build ...
> ```
>
> **Not fixed here:** the empty state still cannot tell "the canister answered with no tables" from
> "the request never completed", and `oisy.js` hardcodes 4943 three more times — filed as
> [T-45](#t-45). Both are `src/cleardeck_frontend/**`, another owner's tree.

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
### H-24 — high — the fairness gate certifies a panel that has not verified anything — STATUS: FIXED (wave 5)

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
### T-16 — high — the phone is where this client loses, and it loses by more than 2× — STATUS: FIXED (wave 4)

> **FIXED 2026-08-05.** Measured with the same tool on the same six captures at the same
> 390×844: the playing surface is now **52.1% of the screen at 6-max** (`table-preflop-mobile`,
> `table-facing-bet-mobile`, `table-showdown-mobile` — 308×556, identical to PokerNow's 52.1%)
> and **42.9–43.2% at 9-max** (`table-allin-mobile`, `table-empty-mobile`,
> `table-sidepots-mobile` — 279–281×505). Bar 22 (≥40%) is cleared on every scene. Bar 23 is
> cleared too: the page is scroll-anchored on mount, `tallest_col_rel` is 0.31–0.89 on every
> capture (never 0.000), and every pod, the pot and all five board slots are inside the frame in
> all six. Bar 24: the stack figure went from an 8.2 px computed font to **16.4 px at 6-max /
> 14.4 px at 9-max** (measured digit ink 12 px, against PokerNow portrait's 8 px).
>
> What changed, all of it in `PokerTable.svelte`'s portrait block plus one responsive rule in
> `index.scss`: the portrait surface aspect went 0.70 → **0.555** and its height cap now sizes
> the FELT rather than only the ring; `--ring-kx` went 1.00 → **0.70**, so the flanking pods sit
> ON the surface as PokerNow's do instead of hanging off it and capping the felt at 70% of the
> phone; the felt became the tall rounded rectangle bar 20 asks for; the four notices render
> **once** per page in portrait (both blocks are still in `+page.svelte`, verbatim — see
> `index.scss` "THE FOUR PLAYER-PROTECTION NOTICES RENDER ONCE PER PAGE ON A PHONE"); the action
> row moved to the bottom of the dock at a 44 px touch height; and the bet-disc and revealed-pair
> placement rules were transposed for a tall surface. **Desktop is byte-identical**: 962×452,
> aspect 2.13, 66.8% of window width, 33.6% area, exactly as before.
>
> Still open, and NOT this defect: the page renders at **0.918 scale** on a 390 px phone because
> `.header-right` needs 424 CSS px and `app.html`'s viewport meta has no `initial-scale`, so
> Chrome shrinks the document to fit. Every glyph is 8.2% smaller than it should be and ~32 px of
> the screen's right edge is blank. Filed as **T-19**.

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

*Resolution taken in wave 4:* neither copy was deleted or shortened. In **portrait only**, exactly
one of the two renders — the footer copy on the table view, the top banner everywhere else — so a
phone always sees all four notices, once, and the 268 px goes to the table. Desktop still renders
both. `make hygiene` is green and every protected phrase count in `README.md` and
`src/cleardeck_frontend/src` is unchanged.

> **REOPENED AND RE-RESOLVED BY THE WAVE-4 COHERENCE PASS, 2026-08-05.** Two things above are
> false as written, and both were measured rather than argued.
>
> 1. **"A phone always sees all four notices, once" was not true on the table view.** The footer
>    copy is the last block of a long document, not a pinned strip. Live DOM at 390×844 on the
>    table view: `.alpha-warning-banner` `display:none`; `.footer-disclaimer` `y 931..1166` in a
>    918 px viewport, **248 px of scroll away**; **0 of the 4 protected phrases on screen**, 4 of 4
>    in the DOM. Identical with the Deposit modal open. See **[T-20](#t-20)**, now fixed: in
>    portrait it is the TOP BANNER that survives, on every view.
> 2. **The 42.9–52.1% headline only exists in the configuration that hid the notices.** With the
>    notices on screen — the only shippable configuration — the same build measures
>    **18.3% (6-max) / 16.4% (9-max)**, against **20.9%** for the geometry this defect replaced.
>    On its own metric, as it must ship, the portrait redesign is a **regression**. Three-variant
>    A/B in [WAVE-04.md §4](WAVE-04.md); the aspect is not the bug, the vertical budget is
>    ([T-19](#t-19)). Filed as **[T-25](#t-25)**.
>
> What *did* survive re-measurement, unchanged: nothing is off-frame at either viewport
> (`offFrame: []` on every probe, bar 23 clear); `scrollY = 0` on arrival; desktop untouched at
> 982.8 × 468, aspect 2.10, 35.5% of window; and the stack digits do beat PokerNow's 8 px.

<a id="t-19"></a>
### T-19 — medium — the phone renders the whole app at 0.918 scale, and 32 px of the screen is blank — STATUS: FIXED (wave 5)

Measured on the live local build at a 390×844 device viewport:

| reading | value |
|---|---|
| `window.innerWidth` | **425** CSS px |
| `document.body` / `header` / `main` width | **390** CSS px |
| widest element on the page | `.header-right`, **424 CSS px** (sound toggle + History + Verify Fair + WalletButton, none of which shrink or wrap) |
| resulting page scale | **390 / 425 = 0.918** |

`src/cleardeck_frontend/src/app.html` declares `<meta name="viewport" content="width=device-width" />`
with **no `initial-scale`**. When the document's content is wider than the layout viewport and no
initial scale is pinned, Chrome zooms the page out until the widest content fits. So the app is
laid out at 390 CSS px, painted into 358 device px, and the remaining ~32 device px of the screen's
right edge is empty background. Every glyph on every phone screen is 8.2% smaller than the value in
the stylesheet, and the blank strip is visible in every mobile capture in `artifacts/screens/latest/`.

**Do not "fix" this with `initial-scale=1` alone.** Un-zooming stops the *fixed-pixel* page chrome
shrinking with the page: the table-view header is 174 CSS px of hard-coded padding and type, so at
scale 1 it costs 174 device px instead of 160, the vertical budget the table is sized from *shrinks*
by 74 CSS px, and the measured playing surface drops from 52.1% to ~43%. The header would also then
overflow 390 px horizontally and be clipped, hiding part of the wallet button.

The fix is two changes together, in files a mobile-layout pass does not own:
1. let `.header-right` wrap or shrink at ≤768 px so nothing forces the document past the device width;
2. give the table view a compact header (the Lobby button, the stakes pill and the wallet chip on one
   ~56 px row) so the vertical budget does not regress when the page stops shrinking.

Until both land, the 0.918 scale is *load-bearing* for the mobile playing-surface numbers in T-16.

**FIXED IN WAVE 5.** Both landed together, and the warning above about `initial-scale=1` alone was
right: the header had to come down in the same change.

| reading | before | after |
|---|---|---|
| `window.innerWidth` × `innerHeight` | 425 × 918 | **390 × 844** |
| page scale | 0.9176 | **1.0000** |
| `documentElement.scrollWidth` on the table view | **425** (overflowing 390) | **390** |
| widest thing in `.header-right` | `.header-right` itself, 411.8 | 374, content 338 |
| `header` height on the table view | 173.5 (three rows) | **69.0** (two rows) |
| everything above the table | 449.5 of 918 (49%) | **132.6** of 844 (16%) |
| 6-max portrait felt | 199.1 × 358.7 = 18.3% | **332.8 × 599.5 = 60.6%** |

The brand row is dropped on the table view only (44 px of a screen where the player is already
inside the product); every control keeps its label; `.header-right` is allowed to WRAP at narrower
widths, so a 320 px phone gets a third header row and a smaller felt rather than a clipped wallet
button — verified `scrollWidth == innerWidth` at 320, 360, 390, 430 and 768 px.

Two consequences worth naming, both of which were invisible before:

* The repo's own pixel-occlusion gate **could not run at all** on two mobile scenes before this fix.
  `page.screenshot({ clip })` on an element whose page rect reaches x≈425 throws
  `Clipped area is either empty or outside the resulting image` against a 390 px painted image, so
  `table-facing-bet` and `table-allin` at mobile crashed the gate rather than passing or failing it.
  Both pass it now.
* The blank ~32 px strip at the right edge is visible in every pre-fix mobile PNG in
  `artifacts/screens/`; it is gone.

<a id="d-03"></a>
### D-03 — medium — there was no design system, so four agents each invented one — STATUS: OPEN

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
### T-15 — medium — the canonical money layer has no consumers — STATUS: OPEN

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
### T-12 — low — the main pot was labelled "Side 1" — STATUS: FIXED (wave 3)

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
### T-13 — low — three of four dialogs ignored Escape — STATUS: FIXED-NO-GATE (wave 3)

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
### H-25 — medium — `known-defects` watches one defect — STATUS: OPEN

```
==> known-defect markers (expected RED until wave 2 fixes them)
    red   defect_detect_straight_returns_the_best_straight
==> result
    1 of 1 engine defects still present
```

One marker, for **E-13**, the lowest-severity entry in this register ("unreachable today"). E-06
(one timeout ends the hand for everybody), E-07 (`admin_reinit_table` stranded chips; its markers
were `reg06` in money-safety and `probe5` in the wave-6 coherence file, both of which PINNED the
destruction as expected behaviour and are now inverted into gates), E-32
(a `Disconnected` seat stays live in the hand), E-33, E-34, E-36 and now E-40 have none. The rule
in the brief — "when you fix a defect, update its marker so `make known-defects` stays meaningful"
— cannot bite on defects that never got a marker. The target reads as a green tick over an almost
empty set.

<a id="t-17"></a>
### T-17 — low — the hand-export button threw on the same BigInt class — STATUS: FIXED-NO-GATE (wave 3)

`HandHistory.svelte`'s "download this hand" builds a payload containing `amount_e8s` and `won_e8s`
straight off the Candid records — `nat64`, i.e. `BigInt` — and called bare `JSON.stringify`. Same
throw as T-10, in a control no scene photographs. Now uses a BigInt replacer that writes e8s as
decimal strings, which is what a `nat64` is on the wire.

Worth noting as a pattern rather than a bug: `+page.svelte` already carries a private
`safeStringify()` doing exactly this, `HandHistory` did not know, and `PokerTable` had a third
variant of the problem. Three components, three answers, two of them wrong.

<a id="t-18"></a>
### T-18 — high — the withdrawal receipt stated the wrong amount of money — STATUS: FIXED (wave 4)

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

---

<a id="attribution-what-was-planted-and-what-convicted-it"></a>
## Attribution: what was planted, and what convicted it

The wave-3 critic's charge against the [E-37](#e-37) / FINDING 13 fix was not that it was wrong.
It was that the gate around it was the shape of the hand it was reverse-engineered from:

* a pot share paid to the **wrong live player** passed all three M8 tests, because the only one that
  ran on an ordinary hand — "a principal who staked nothing cannot come out richer" — exempts
  everybody who did stake;
* `push_winner`'s owner-keyed aggregation was defended by a comment and by nothing else;
* M8 was absent from the fuzzer's per-step loop entirely.

**What changed.** Attribution is now a property of every settled hand, on four legs
(`tests/money_safety/src/invariants/attribution.rs`), measured by a recorder fed the snapshot the
fuzzer already takes (`tests/money_safety/src/hand_attribution.rs`):

| leg | statement | needs an oracle? |
|---|---|---|
| `value_delta_matches_the_recorded_award` | each person's `escrow + chips` moved by exactly what the canister's OWN hand history says it paid them, minus what they staked | no |
| `value_delta_matches_the_settlement_oracle` | ...and by exactly what the RULES of poker owe them. The rules are `settlement_oracle::oracle`, the independent derivation from `tests/settlement`, so there is one statement of them in the repo | yes |
| `no_free_money_for_a_principal_that_staked_nothing` | you cannot win money from a hand you did not play | no |
| `a_relinquished_stake_cannot_profit` | somebody who folded, or who left the chair, can be handed back money nobody covered and nothing else | no |

and `tests/settlement` gained a third dimension beside its per-seat and per-principal columns: a
per-RECORD column comparing the winner list the canister PUBLISHES, by principal, against the
oracle. `HandComparison::agrees` — the thing `Bench::gate` fails every hand on — now includes it.

### The matrix

Six misdirection bugs planted one at a time in `src/table_canister/src/lib.rs` in a scratch clone,
each one CONSERVING (no chip created, no chip destroyed, the plan still awards what it collected),
so nothing aggregate can see any of them. "before" is a pristine `git archive HEAD` tree with the
wave-3 harness; "after" is the same engine mutation against the widened gate.

| # | planted bug | before | after | convicted by |
|---|---|---|---|---|
| P1 | a pot share paid to the **wrong live player** (`plan_payouts` names another live claim's owner; the record names the same wrong person) | settlement only | **all three** | `m8_every_ordinary_hand_pays_the_people_the_rules_of_poker_name`, `m8_heads_up_hands_pay_the_right_person`, fuzzer `value_delta_matches_the_settlement_oracle`, three settlement tests |
| P2 | **two winners' amounts swapped**, in the money and in the record together | settlement only | settlement only | `randomised_hands_settle_the_way_the_rules_of_poker_do`, `the_oracle_convicts_a_conserving_wrong_seat_payout`. See [H-27](#h-27) |
| P3 | **`push_winner` merges two principals** at one chair (money untouched — `credit_escrow` has already run — only the published record is wrong) | **NOTHING. Escaped every gate in the repo** | hand-written M8 | `m8_a_chair_with_two_owners_credits_each_of_them_under_their_own_name`. See [H-27](#h-27) |
| P4 | the right principal's **record**, the wrong seat's **stack** | settlement only | **all three** | two ordinary-hand M8 tests, all four fuzzer legs, three settlement tests |
| P5 | a **folded player who is still seated** can win the pot | settlement only | **fuzzer + settlement** | fuzzer `a_relinquished_stake_cannot_profit` and `value_delta_matches_the_settlement_oracle`, two settlement tests |
| P6 | **pay whoever is in the chair** — FINDING 13 restored in one line, which is what a player who leaves and returns as a different principal walks into | hand-written M8 + settlement | hand-written M8 + settlement | `m8_a_departed_stake_is_paid_to_its_owner_and_not_to_whoever_took_the_chair`, `m8_a_chair_with_two_owners_...`, `a_chair_that_changes_hands_mid_hand_pays_the_person_and_not_the_chair` |

**Escapes: none.** Every plant is convicted by at least one suite, and P3 — the one the critic
named — went from convicted by nothing to convicted by a deterministic fixture. What moved most is
the money-safety harness: it caught **1 of 6 before and 4 of 6 after**, and the fuzzer went from 0
of 6 to 3 of 6 on randomised hostile sequences rather than on hand-written ones. The two remaining
single-suite convictions are named precisely in [H-27](#h-27).

**Reproduce.** The plants and the three-gate runner are in
`$SCRATCH/attribution-gate/{plant.py,gates.sh}`; each plant is a one-anchor string replacement
against the checked-out `lib.rs`, so a reader can re-derive every row by hand. The gates are:

```text
G1  cd tests/money_safety && cargo test --test invariants -- m8_
G2  cd tests/money_safety && MONEY_FUZZ_SEEDS=212967420072195,212967420072196,212967420072197 \
                             MONEY_FUZZ_STEPS=300 cargo test --test fuzz -- --nocapture
G3  cd tests/settlement    && cargo test --test settlement -- --test-threads=1 \
        randomised_hands_settle_the_way_the_rules_of_poker_do \
        a_chair_that_changes_hands_mid_hand_pays_the_person_and_not_the_chair \
        the_oracle_convicts_a_conserving_wrong_seat_payout
```

All three are green on the clean tree at `table_canister.wasm`
sha256 `5996594741afd63f88f38098c5461a24fbacafbda16278fc44bd3317a6c55c23`, toolchain 1.90.0.

---

## Found by the wave-4 coherence pass

Wave 4 built five things in parallel and **three of them edited
`src/cleardeck_frontend/src/lib/components/PokerTable.svelte`**. Everything below came out of
reconciling that file and then **walking the whole app once, at both viewports, in a real browser
against the real local canisters** — land signed out, sign in, enter a table, open Deposit, take a
seat, play a hand to a showdown by clicking the app's own buttons, open the hand history, open the
per-hand replayer, verify the shuffle, open Withdraw. Both walks completed with **zero page errors
and zero console errors**. Script and raw output: `$SCRATCH/walk.mjs`,
`$SCRATCH/walk/report-{desktop,mobile}.json`.

Full narrative, gate results and the scene-by-scene A/B are in **[WAVE-04.md](WAVE-04.md)**.

> **HISTORICAL SNAPSHOT — the status column below is NOT maintained.** It is the queue as it was written in this wave. The only true statuses are in [THE REGISTER](#the-register).

| # | sev | status | where | one line |
|---|---|---|---|---|
| [T-20](#t-20) | **high** | **FIXED IN THIS PASS** | `src/index.scss` portrait rule | on a phone, on the table view and behind the open Deposit modal, **0 of the 4 protected notices were on screen** (4 of 4 in the DOM, 248 px of scroll away). `make hygiene` cannot see this: it greps the source |
| [T-21](#t-21) | **high** | **FIXED IN THIS PASS** | `HandHistory.svelte:582-593` | the per-hand replayer asserted *"These cards were fixed before the hand was played … before any card was dealt"* with a green tick, in the same wave and the same product as `ShuffleProof.svelte`'s *"Not proven: that the commitment came before the cards"* |
| [T-25](#t-25) | **high** | open | the portrait table as a whole | with the notices restored, the wave-4 portrait redesign measures **18.3% / 16.4%** of the screen against **20.9%** for the geometry it replaced. It is a regression in the only shippable configuration |
| [T-22](#t-22) | **high** | **FIXED IN WAVE 5** (measured red then green, on the rendered page) | `.equity-badge` in portrait vs `.player-cards` | on a phone the winner's `100.00%` rendered as **`0%`** — the hero's own card covered the rest. The badge is out of the plate's stacking context and on a per-seat spoke no card of that seat uses: **60.4% / 46.8% of its ink covered before, 0.0% after** |
| [T-23](#t-23) | medium | **FIXED IN WAVE 5** (measured red then green) | `.winner-award` | the award chip covered the winner's revealed pair (**68.1% of a card's ink, 58.8% of its rank glyph**) and, on desktop, a community card's suit pip (**5.2%**). Root cause: one multiplier scaled the chip vector, which points AT the board at flank seats. The award has its own per-seat vector now; **0.0% after** |
| [T-24](#t-24) | medium | **FIXED IN THIS PASS** | `HandHistory.svelte:1318` | below 560 px the action log hid `.log-seat`, so every line on a phone read `06:03:57 AM calls` — 8 of 8 anonymous |
| [T-26](#t-26) | medium | **FIXED IN THIS PASS** | `WithdrawModal.svelte:15` vs `:167,:179` | the BTC withdrawal minimum the modal **stated** (1,000 sats) was **90.9× the one it enforced** (11 sats), and the error string it printed was unreachable for 12–999 sats. Resolved to **11**, the canister's number, on every surface: a UI-only floor of 1,000 would have trapped any balance below it. Gated by `tests/money_safety/tests/ui_limits.rs`, which reads `lib.rs` and both modals |
| [T-31](#t-31) | **high** | **executed, and FIXED for the two money modals in this pass** | `.modal-backdrop` in `WithdrawModal.svelte` / `DepositModal.svelte` vs the notice banner | with **either money modal open, 0 of the 4 protected notices are unobstructed**, at 1440x900 AND at 390x844. `elementFromPoint` at the centre of each returns `.modal-backdrop` — `rgba(0,0,0,0.7)` + `backdrop-filter: blur(4px)`, z-index 200. Identical on `fe72d46`, so the wave-4 T-20 fix left this case open. All four are now restated INSIDE both dialogs: **4 of 4 unobstructed** on both viewports with either modal open |
| [T-28](#t-28) | medium | **FIXED IN THIS PASS** | `WithdrawModal.svelte` `setMaxAmount` | MAX put an amount in the box that could not be withdrawn, two ways: `toFixed(4)` rounded a 123,456,789 e8s balance UP to 123,460,000 and the modal refused its own MAX with `Insufficient balance`; and it ignored the 100 ICP per-transaction ceiling, so 500 ICP produced a canister rejection from a button labelled MAX. Floors and clamps now |
| [T-29](#t-29) | medium | **FIXED IN THIS PASS** | `WithdrawModal.svelte` vs `lib.rs:47,51,54` | two limits the canister enforces and no surface mentioned: the **100 ICP / 0.1 BTC per-transaction ceiling** and the **60-second withdrawal cooldown**. A player met both as an unexplained rejection. Both now stated from the mirrored constants; the ceiling is checked client-side too |
| [T-30](#t-30) | medium | **FIXED IN THIS PASS** | `DepositModal.svelte:399,634` | the "you have enough to deposit" test was `balance > minDeposit`, but an ICRC-2 deposit costs the depositor **two** ledger fees, so the real floor is `minDeposit + 2 × fee`. A wallet with 1,005 sats was shown a form whose every possible deposit the ledger would refuse |
| [D-05](#d-05) | low | executed | `DepositModal.svelte` native-BTC path | the **10,000 sat** minimum and **~2,000 sat** cost the native-BTC flow states are the ckBTC **minter's**, and no constant in this repository enforces either, so `ui_limits.rs` cannot check them. Reduced to two named constants so they cannot disagree with each other; they can still disagree with the minter |
| [T-27](#t-27) | low | **FIXED IN WAVE 5** | `.equity-method` inside `.pot-display` | the line that names the equity method disappeared exactly when the equity became a verdict, because the pot display is replaced by the winner banner. It is one snippet rendered into whichever readout is on screen; measured in the same frame as the badges at both viewports |
| [H-30](#h-30) | medium | open | `ShuffleProof.svelte` placement + `shuffleproof` scene | the green verdict is **below the fold at both viewports**; on mobile it is inside a nested scroller (`.proof-sidebar`, `clientHeight 918`, `scrollHeight 2708`) that scrolling the page cannot reach |
| [H-31](#h-31) | medium | **FIXED IN THIS PASS** | `tools/shots/scenarios/handreplay.mjs` (new) | the replayer is now opened, walked street by street and asserted at both viewports: every board card against `community_cards`, every log line against the hand record, the fairness claims, and the four protected notices hit-tested on the rendered page |
| [H-32](#h-32) | **high** | **FIXED IN THIS PASS** | `src/declarations/table_1/table_1.did.js` | the client's Candid declaration of `ActionRecord` omitted `phase` and `amount`, so the agent decoded three fields off a five-field record. **No client could show a call's or an all-in's amount, or the street any action happened on, however it was written.** Root cause is [E-08](#e-08) (the hand-maintained `.did`), which is a different owner's file and stays open |
| [H-33](#h-33) | medium | **FIXED IN THIS PASS** | `tools/shots/scenarios/handhistory.mjs` staging wait | `querySelectorAll('.community-cards .card').length >= 5` was **true the instant the board frame mounted**: `PokerTable.svelte` always renders `Array(5)` of `<Card>` and an undealt slot is still a `.card`. A wait that cannot fail stood in for the one piece of state the scene depends on |
| [H-34](#h-34) | medium | **FIXED by the gate's owner, 2026-08-05** | `tools/shots/lib/occlusion.mjs` | the new pixel gate had no notion of a dialog, so **every modal scene failed it**, the pre-existing `handhistory` scene included: a dialog covering the table behind it is what a dialog is for. The gate now identifies the overlay LAYER (fixed ancestor covering ≥40% of the viewport, or `role="dialog"`/`aria-modal`) and reports a page figure covered by an overlay as `behind-an-overlay` instead of failing; a figure INSIDE an overlay is still gated, so a dialog covering its own numbers still fails. `handhistory` and `handreplay` are verified at both viewports with 0 occluded and no `SHOTS_OCCLUSION=report` |
| [H-35](#h-35) | medium | **FIXED IN THIS PASS** | `HandHistory.svelte` proof panel, download file, list foot | three claims [T-21](#t-21) left behind in the same modal: the commitment row was labelled *"committed before the deal"*, the downloadable audit file called the field `commitment_published_before_deal`, and the list foot said *"**Every** hand above was re-derived in this browser"* while listing hands whose seed is still sealed |
| [H-36](#h-36) | **high** | **FIXED for this dialog; open for the others** | every full-screen dialog vs `.alpha-warning-banner` | wave 4's *"4 of 4 phrases on screen … **and behind the open Deposit modal**"* is a GEOMETRY measurement. Hit-tested, a dialog's 72%-black scrim hides all four: with the hand-history modal open and its own copy of the notices suppressed, **0 of 5** protected phrases are on screen while **4 carriers sit in the DOM** |
| [D-04](#d-04) | medium | **FIXED IN THIS PASS** | `docs/DESIGN-BAR.md` §9.4.1 | "There is no real mobile lobby capture in the corpus" is false. `pokerstars/web-ps-gipsy-2.png` is one, indexed `real_gameplay=true`; the doc missed it by querying `scene == 'lobby-mobile'` when it is filed `mobile-portrait` |

<a id="t-20"></a>
### T-20 — high — the four protected notices were on screen zero times where a player spends money — STATUS: FIXED (wave 4)

**Reproduce (before the fix).** Deploy `3253b67` + the wave-4 frontend, open the app at 390×844,
sign in, enter any table, and read the DOM:

```js
for (const sel of ['.alpha-warning-banner', '.footer-disclaimer']) {
  const el = document.querySelector(sel), cs = getComputedStyle(el), r = el.getBoundingClientRect();
  console.log(sel, cs.display, Math.round(r.top), Math.round(r.bottom), window.innerHeight);
}
```

Measured, shipped build, table view (layout viewport 425×918 because of [T-19](#t-19)):

| block | display | box | on screen |
|---|---|---|---|
| `.alpha-warning-banner` | `none` | — | no |
| `.footer-disclaimer` | `block` | `y 931..1166`, h 235 | **no — 248 px of scroll away** |

Phrases on screen: `Unaudited code with known bugs` **NO**, `your funds are NOT safe` **NO**,
`18+ only` **NO**, `illegal in many jurisdictions` **NO**. All four in the DOM. **Identical with the
Deposit modal open**, which is the screen a player commits real ICP from. At `3253b67` the same
probe reports the banner at `y 0..268`, fully in the viewport.

The wording was never touched — both blocks are byte-identical to `3253b67` and `make hygiene` was
green throughout, because hygiene greps the **source** for the four phrases and a notice can be
present in the bundle and absent from the screen.

**Fixed.** The sanctioned de-duplication is kept — a phone renders the four notices once, not twice
— but the copy that survives in portrait is now the top banner, on every view, instead of the
footer. One rule replaced two in `src/index.scss`:

```scss
@media (max-aspect-ratio: 1/1) { .footer-disclaimer { display: none; } }
```

Verified after the fix, same probe: banner `y 0..268`, fully in the viewport, **4 of 4 phrases on
screen**, on the mobile table view and behind the open Deposit modal. Desktop unchanged and still
renders both copies. What it costs the felt is [T-25](#t-25), stated in full rather than traded
against the notice.

<a id="t-21"></a>
### T-21 — high — two trust surfaces of the same product made opposite claims about the same fact — STATUS: FIXED (wave 4)

`ShuffleProof.svelte` was corrected this wave to say, of the commit-before-deal ordering:

> **Not proven.** *That the commitment came before the cards.* This page reads … off the table
> canister; it did not watch the order of events.

`HandHistory.svelte`'s per-hand replayer — the surface a player opens to review a hand they lost —
still opened with a green tick and the same claim at full strength. Captured live at both
viewports; the word "proven" appeared in the modal **zero** times. In the same modal the first
action-log line is timestamped **five seconds after** the moment the copy calls "before any card was
dealt", so the page holds the evidence that it is reading a clock.

**Fixed.** The banner now states what the browser actually did and carries the ordering as a tagged
`NOT PROVEN` limit **inside** the same green box, so the verdict cannot be read without the caveat,
together with the one action that would settle it (copy the commitment mid-hand, compare after the
reveal). Verified live at both viewports: `notProvenInModal: true`.

<a id="t-25"></a>
### T-25 — high — with the notices on screen, the portrait redesign is a regression on its own metric — STATUS: FIXED (wave 5)

One tool, three variants, same build, same replica, two tables, `getBoundingClientRect` on `.felt`
over the frame, mobile 390×844. Variant C reproduces `3253b67`'s portrait geometry by injecting its
CSS constants, which is exact for the felt box because that box is a pure function of `--ar` and
`--fw`. Script: `$SCRATCH/felt-ab.mjs`.

| variant | notices on screen | 6-max | 9-max |
|---|---|---|---|
| **A — shipped now** (wave-4 geometry, notices restored) | 4 of 4 | 199.1×358.7 = **18.3%** | 188.2×339.2 = **16.4%** |
| B — wave 4 as built (notices hidden) | 0 of 4 | 335.4×604.3 = 51.9% | 304.2×548.1 = 42.7% |
| C — `3253b67` geometry (notices on screen) | 4 of 4 | 238.9×341.3 = **20.9%** | 238.9×341.3 = **20.9%** |
| PokerNow `mobile-portrait-1`, real capture | n/a | 313×548 = **52.1%** | — |

**The aspect is not the bug.** 0.555 is the right shape when there is height to spend, and there is
not: 268 px of banner plus a 173.5 px three-row table header on a 918 px layout viewport leaves the
*height* cap binding, and at a fixed height a narrower felt has less area. At 6-max variant A's felt
is 18 px taller and 40 px narrower than variant C's, and loses on area for exactly that reason.

**Fix order, neither of which touches a notice:** (a) [T-19](#t-19) — `initial-scale`, a
`.header-right` that fits and a two-row table header return ~120 CSS px and 8.2% of linear scale;
(b) make `PORTRAIT_AR` a function of the stage height rather than a constant.

**FIXED IN WAVE 5 by (a) alone. (b) was never needed, and the reason is worth recording.** Same
tool, same replica, same six scenes, mobile 390×844, notices on screen and hit-tested throughout:

| variant | notices on screen | 6-max | 9-max |
|---|---|---|---|
| A — wave 4 as shipped | 5 of 5 | 182.7×329.2 = **18.3%** | 172.7×311.2 = **16.3%** |
| C — `3253b67` geometry | 4 of 4 | 238.9×341.3 = 20.9% | 238.9×341.3 = 20.9% |
| **wave 5** | **5 of 5** | **332.8×599.5 = 60.6%** | **304.2×548.1 = 50.7%** |
| PokerNow `mobile-portrait-1`, real capture | n/a | 313×548 = 52.1% | — |

`--ar: 0.555` is untouched. Once 317 CSS px of chrome came back (449.5 → 132.6 above the table, and
the page stopped painting at 0.918), the **width** cap started binding instead of the height cap, and
at the width cap a 0.555 felt is the largest the screen can hold. That is exactly what §"WHY 0.555
AND NOT 0.70" in `PokerTable.svelte` predicted; the constant was right and the budget was wrong.

The remaining headroom is small and is stated so nobody re-litigates it: at 6-max the height cap
still binds by 2.6 px of felt width (`min(86cqw, 55cqh)` = `min(335.4, 332.8)`), worth **1.0 point**
of area, and the `55cqh` in that expression is itself a round-in of an exact `55.5cqh`. At 9-max the
**width** cap binds (`min(78cqw, 52cqh)` = `min(304.2, 314.6)`), so 50.7% is the ceiling for a nine-pod
ring at `--ring-kx: 0.90` and no vertical budget can improve it.

<a id="t-22"></a>
### T-22 — high — on a phone the winner's `100.00%` renders as `0%` — STATUS: FIXED (wave 5)

**Status FIXED 2026-08-05, measured on the rendered page before and after.** The badge is no
longer inside the plate's stacking context: it is a child of `.seat` at `z-index: 9`, above the
cards, and it hangs off the plate on that seat's **readout spoke** — the one side of the plate
that neither this seat's cards nor its bet disc nor the board nor the pot readout claims
(`--rdx`/`--rdy`, computed per seat and per orientation in `ringSeats`, see the comment there).
Clear of the cards by construction rather than by luck, at both viewports.

Two placements were tried and measured first. Leaving the badge above the pod's corner and
merely raising its z-index only reverses who covers whom — it then sits on the hero's own card.
Hanging it off the outward normal puts it off the felt at a landscape bottom seat (the same
failure that killed the old "All In" tag: read at y=818 on a surface ending at y=753) and off
the SCREEN at a portrait flank seat.

The badge also stopped costing the pod any width: `.pod-slot.with-equity { min-width: 3.5em }`
reserved 39–59 px of a 116 px phone pod, and what lost that argument was the stack figure.

**Before → after, both from the pixel gate, on the real canisters.** The "before" column is a
build of `fe72d46`'s `PokerTable.svelte` served through `SHOTS_SERVE_DIST` so that every
canister call, every scene and every threshold is identical between the two columns:

| scene, viewport | figure | before | after |
|---|---|---|---|
| `table-allin` mobile | hero's `43.4%` badge | **60.4% of its ink covered** by `.player-cards.hero` (180 of 298 px; 8 of 15 hit-test probes answer something else) | 0.0%, scene verified |
| `table-showdown` mobile | hero's `0.00%` badge | **46.8% covered** by `.player-cards.hero` (357 of 763 px) | 0.0%, scene verified |
| `table-allin` desktop | badges | 0.0% (T-22 is portrait-only, as reported) | 0.0% |

Note the second row: `elementFromPoint` at that badge's centre answered `span.equity-badge`
itself while 46.8% of it was painted over. Hit testing alone under-reports; the four-shot pixel
differential is what decides.

**The original report follows.**

In the gate's own accepted artifact `artifacts/screens/latest/table-showdown-mobile.png` the hero's
equity badge says `0.00%` and a player sees **`0%`**, because the hero's own `8♠` covers the rest of
it. Confirmed in the live DOM rather than by eye — `document.elementFromPoint` at the centre of each
badge returns `div.player-cards` and `span.pip` on **both** badges, at both the 6-max and 9-max
tables, at the showdown and at the all-in.

**Root cause is a stacking collision between two of the three PokerTable editors.** Inside a
`.seat`, `.player-nameplate` is `z-index: 6` and therefore opens a stacking context; the mobile pass
placed `.equity-badge` *inside* it at `z-index: 3`; the all-in pass draws revealed and hero cards at
`z-index: 7`. From where it is, the badge can never win.

**Not fixed here.** Every candidate fix moves either the plate or the badge's containing block, and
a coherence pass is not authorised to redesign the portrait pod. Wave 5, item 1. The durable half is
a **pixel** assertion in the gate — fail the scene if any element with a higher effective z-index
intersects `.equity-badge`'s client rect — because every existing gate reads `textContent` and
therefore cannot see any of this.

<a id="t-23"></a>
### T-23 — medium — the award chip covers the cards that justify it — STATUS: FIXED (wave 5)

**Status FIXED 2026-08-05.** The root cause is one multiplier meaning two different things.
The award was the chip spot times `--award-out` (1.30 landscape, 1.12 portrait), and the chip
spot uses whichever axis has room: the **tangent** at top and bottom seats in landscape, the
**normal** at flank seats. Scaling a tangent moves the award further from the board; scaling a
normal drives it **into** the board, because the normal points at it. In portrait it is worse
still: at a flank seat the chip's tangent and the cards' `cy` are the *same direction*, so the
award landed on the winner's own revealed pair.

The award now has its own per-seat vector (`ax`/`ay` in `ringSeats`, with the reasoning in the
comment there):

* landscape, tangent seats — unchanged, 1.30× the chip spot, which was already proven;
* landscape, flank seats — keep the chip's own inward distance, take the extra clearance along
  the tangent, away from the board's row;
* portrait, every seat — the award rides the **readout spoke** with the equity badge, at the
  other end of it (vertical spoke) or one step further out along it (horizontal spoke). The
  `--award-dx`/`--award-dy` pair is named once so the landing animation cannot drift from the
  resting position, which it previously could: the keyframes restated the vector.

**Before → after, same method as [T-22](#t-22).** The award reaches the board only when the
winner sits at a flank seat, so the desktop row below is a hand where seat 2 won:

| scene, viewport | figure | before | after |
|---|---|---|---|
| `table-showdown` desktop | community card's `♠` pip | **5.2% of the pip's ink covered** by `.winner-award` (50 of 965 px; the award's rect covers 19% of the pip's) | 0.0%, scene verified |
| `table-showdown` mobile | winner's revealed `Q♠` | **68.1% of the card's face covered** (821 of 1206 px) | 0.0%, scene verified |
| `table-showdown` mobile | that card's `Q` rank glyph | **58.8% covered** | 0.0% |
| `table-showdown` mobile | two revealed `♠` pips | **48.8% and 46.1% covered** | 0.0% |
| `table-showdown` mobile | winner's `4♠` | **37.6% covered** | 0.0% |

The desktop figure is smaller than the 25.6% in the original report because the two are
different denominators: 25.6% was the fraction of the AWARD's own area that overlapped
`.community-cards`; 5.2% is the fraction of the PIP's own ink that stopped being visible, which
is the number that says what the player lost.

**The original report follows.**

Mobile, same artifact: the `+24.00` chip sits on the winner's revealed pair (the red `8` of a
revealed card is visible behind it). Desktop, measured live: `.winner-award` overlaps
`.community-cards` by **25.6% of the award's own area** when the winner is at seat 1 or 2, and by 0%
when the hero at seat 0 wins — which is why one run of the walk saw it and one did not. Same family
as [T-22](#t-22) and the same fix applies.

<a id="t-24"></a>
### T-24 — medium — the mobile action log named nobody — STATUS: FIXED (wave 4)

`HandHistory.svelte`'s `@media (max-width: 560px)` block set `grid-template-columns: 54px 1fr` and
`.log-line .log-seat { display: none }`. Measured on a hand I played myself at 390×844: 8 of 8 log
lines read `06:03:57 AM calls` — no actor at all, so the log could not answer the only question it
exists to answer. Desktop showed `Seat 2 calls` on the same hand.

**Fixed**: the monospace time column gives up 10 px instead (`52px 46px 1fr`, 11 px type). Verified:
actor present on **8 of 8** lines on a phone. That the actor still reads `Seat 2` while the felt
three inches away says `Nakamoto` is a separate, older gap, and is in the wave-5 list.

<a id="t-26"></a>
### T-26 — medium — Withdraw stated a BTC minimum 90.9× the one it enforced — STATUS: FIXED (wave 4)

What was there at `fe72d46`:

```
WithdrawModal.svelte:15   const minWithdrawal = isBTC ? 11n : 100000n;
WithdrawModal.svelte:167  min={isBTC ? (inputUnit === 'sats' ? "1000" : "0.00001") : "0.001"}
WithdrawModal.svelte:179  Minimum: 1,000 sats (Fee: 10 sats)
WithdrawModal.svelte:60   const minDisplay = isBTC ? '1,000 sats' : '0.001 ICP';
```

The enforced floor was **11 sats**; every surface a player read said **1,000**. The error string
`Minimum withdrawal is 1,000 sats` was unreachable for any amount from 12 to 999 sats. ICP was
consistent (0.001 ICP = 100,000 e8s), and BTC tables are local-only today, which is the only reason
this is `medium`. No denominator in the money-figure census contained it, because the census walks
text nodes and never reads an input's `min`.

#### Which number is right: 11, and the reasoning matters more than the number

11 is defensible only barely — `BTC_MIN_WITHDRAWAL_AMOUNT = 11` sits one satoshi above the 10-sat
`CKBTC_TRANSFER_FEE`, so a withdrawal at the floor pays the player **1 sat**. That is technically
valid and practically useless, and it is exactly why somebody wrote 1,000 in the copy. It is still
the right number for the UI to state, for three reasons in order of weight:

1. **The canister is the enforcement.** `withdraw()` compares against
   `Currency::min_withdrawal()` and nothing else does. A player calling the canister directly gets
   11 whatever the modal says. A UI that states a floor the canister does not apply is lying in the
   only direction that produces unreachable error strings.
2. **Enforcing 1,000 client-side would TRAP DUST.** A BTC player who loses down to 400 sats has
   exactly one exit — `withdraw` — and a UI-only floor of 1,000 closes it permanently. The canister
   would have paid them 390 sats. Choosing the *higher* number costs a player their whole remaining
   balance, which is a worse defect than the one being fixed.
3. **Raising the canister constant to 1,000 is the same trap**, written into a canister that
   custodies real funds, and it is not a change to make from a frontend pass. (`src/table_canister`
   is untouched by this pass; the module hash is unchanged.)

The real complaint behind "1,000" is answered honestly instead of with a false floor. The modal now
states the fee, computes the **net the wallet will actually receive** from whatever is typed, and
says so louder when the fee takes half or more:

```
BTC table:  Minimum 11 sats, maximum 10,000,000 sats per transaction. The network fee is
            10 sats and is taken out of what you withdraw, so your wallet receives that
            much less. One withdrawal every 60 seconds.
            entered 11   -> "Your wallet receives 1 sats after the 10 sats fee.
                             The fee is most of this withdrawal."
            entered 1000 -> "Your wallet receives 990 sats after the 10 sats fee."
ICP table:  Minimum 0.001 ICP, maximum 100 ICP per transaction. The network fee is
            0.0001 ICP ...
```

A player can still withdraw 11 sats. They can no longer be surprised by what arrives.

#### How it is gated

Every figure the modal states or enforces now comes from ONE fenced block of mirrored constants, so
there is no second copy of a number to disagree with the first. `MIN_WITHDRAWAL`, `MAX_WITHDRAWAL`,
`TRANSFER_FEE` and `WITHDRAWAL_COOLDOWN_SECS` are declared inside
`// >>> MIRRORED-LIMITS-BEGIN … // <<< MIRRORED-LIMITS-END`; the hint, the error strings, the
input's `min` and `max` attributes and the net line are all interpolations of them.

`tests/money_safety/tests/ui_limits.rs` makes two separate claims, because either alone is
escapable:

| claim | what it catches | how it is known to fail |
|---|---|---|
| the fenced mirrors equal the constants in `src/table_canister/src/lib.rs` | a canister constant changed without the UI | run in a scratch clone with `MIN_WITHDRAWAL` set to `1000n`: `states a BTC MIN_WITHDRAWAL of 1000 while the canister enforces BTC_MIN_WITHDRAWAL_AMOUNT = 11` |
| outside the fence, no line mentioning a minimum, maximum, fee or cooldown may carry digits outside `{...}` | **T-26 itself** — the mirror was already right and the copy beside it was wrong | run in a scratch clone on the `fe72d46` modals: it names `WithdrawModal.svelte:179 Minimum: 1,000 sats (Fee: 10 sats)`, `:181 Minimum withdrawal: 0.001 ICP` and five more in `DepositModal.svelte` |

Both failures are reproduced permanently inside the binary as well
(`the_literal_check_convicts_the_defect_it_was_written_for`,
`the_mirror_comparison_convicts_the_defect_it_was_written_for`) so the gate cannot become a
tautology once the defect is gone. It is a source-level check and it does not pretend to be
anything else: it is not a claim that a player SEES the right number, it is a claim that the number
a player sees cannot be a *different* number from the one the canister applies, because there is
only one number in the file. It needs no replica and no wasm — two file reads — and it runs in
`make test`.

<a id="t-31"></a>
### T-31 — high — with either money modal open, 0 of the 4 protected notices were unobstructed — STATUS: FIXED (wave 4)

**Found by measuring, which is the only reason it was found.** This pass drove the real app in a
real browser against the real local canisters — sign in with the app's own Dev Login, click into a
table, open Withdraw and Deposit — and asked, for each of the four protected phrases,
`document.elementFromPoint` at the centre of the element that renders it.

| build | view | phrases unobstructed |
|---|---|---|
| `fe72d46` | table view, nothing open | 4 of 4 (desktop and mobile) |
| `fe72d46` | **Deposit open** | **0 of 4** (desktop and mobile) |
| `fe72d46` | **Withdraw open** | **0 of 4** (desktop and mobile) |
| this pass | **Deposit open** | **4 of 4** (desktop and mobile) |
| this pass | **Withdraw open** | **4 of 4** (desktop and mobile) |

What covered them, byte-identically on both builds (the class hash is the same, `svelte-hv5b71`, so
the CSS is literally unchanged):

```
covered by -> class='modal-backdrop svelte-hv5b71'
              background: rgba(0, 0, 0, 0.7)
              backdrop-filter: blur(4px)
              z-index: 200
```

All twelve text nodes stayed in the DOM and inside the viewport the whole time. They were behind a
70%-opaque black scrim with a 4 px blur, on 11–12 px type. That is not "on screen" in the sense
HARD RULE 2 means it, and it is the same shape as [T-20](#t-20), which wave 4 reported fixed — the
T-20 fix made the top banner the surviving copy, which is right, and the banner is exactly what the
backdrop covers. `make hygiene` was green throughout, because it greps the source.

**Fixed for the two money modals, and only there.** The four notices are now restated inside both
dialogs, as the first block of the modal body:

> **Unaudited code with known bugs: your funds are NOT safe.** Online gambling is illegal in many
> jurisdictions. 18+ only. No middleman, no house, 0% rake.

Measured after the change, on the rendered page, at both viewports, with each modal open: **4 of 4
unobstructed and topmost**, at 12 px, at y = 140–243 in an 844 px / 900 px viewport — and the amount
input, the limits hint and the primary button all still `inViewport && topmost` as well. Zero page
errors on either walk.

Why this way and not by raising the banner: the banner lives in `routes/+page.svelte` and
`src/index.scss`, which had other owners this wave, and raising a z-index past the backdrop risks
putting the banner over the dialog itself. Restating the notices inside the dialog needs nothing
outside these two components, holds whatever any future backdrop does, and puts the warning on the
one screen where a player is about to move real money. More prominent is always allowed.

**Still open, and named here so it is not lost:** every OTHER modal in the app has the same
backdrop. `HowItWorks`, the hand replayer, `SoundSettings` and anything else that renders
`.modal-backdrop` will scrim the banner the same way. The general fix is one z-index decision about
the banner, in files this pass does not own. The measurement script is in the wave scratch
(`notices.mjs`) and takes about a minute to point at another dialog.

<a id="t-28"></a>
### T-28 — medium — MAX produced an amount the canister or the balance refuses — STATUS: FIXED (wave 4)

Same file, same class, found while fixing T-26. `setMaxAmount` had two independent ways to put a
number in the box that could not be withdrawn:

```
WithdrawModal.svelte:110  const maxDisplay = maxSmallest / 100_000_000;
WithdrawModal.svelte:111  withdrawAmount = isBTC ? maxDisplay.toFixed(8) : maxDisplay.toFixed(4);
```

1. **`toFixed(4)` rounds, and rounding goes up half the time.** A balance of 123,456,789 e8s
   became `"1.2346"`, i.e. 123,460,000 e8s — **3,211 e8s more than the player has** — and the modal
   then refused its own MAX with `Insufficient balance`. Measured; the numbers above are the actual
   values. It floors now, via the same exact 8-decimal formatter the limits use, so MAX is at worst
   1 e8 under the balance and never over it.
2. **It ignored `*_MAX_WITHDRAWAL_PER_TX`.** A player holding 500 ICP got `"500.0000"` in the box
   and `Maximum withdrawal per transaction is 100.0000 ICP` from the canister, from a button
   labelled MAX. It is clamped to `MAX_WITHDRAWAL` now: 500 ICP gives `100`.

`DepositModal.svelte`'s MAX had the same `toFixed(4)` rounding, but its 2×-fee holdback absorbed
the ≤5,000 e8s of round-up, so it was latent rather than live. It floors now too, and the holdback
is derived from `TRANSFER_FEE` instead of written out as `20 / 20000`.

<a id="t-29"></a>
### T-29 — medium — two limits the canister enforces and the UI never mentioned — STATUS: FIXED (wave 4)

The opposite direction of T-26, and the same complaint from the player: a rejection with no
forewarning.

| limit | canister | what the UI said |
|---|---|---|
| `ICP_MAX_WITHDRAWAL_PER_TX` / `BTC_MAX_WITHDRAWAL_PER_TX` | 100 ICP / 0.1 BTC per transaction, `lib.rs:47,51` | nothing, on any surface, and the client did not check it either |
| `WITHDRAWAL_COOLDOWN_NS` | one withdrawal per 60 s, `lib.rs:54` | nothing. First a player learns of it is `Please wait 47 seconds before withdrawing again` |

Both are now stated in the withdraw hint, interpolated from the mirrored constants, and the ceiling
is checked client-side as well — which cannot trap funds, since a larger balance simply comes out in
successive withdrawals. The cooldown is stated but deliberately not enforced client-side: the
canister's own message carries the remaining seconds, which a mirror cannot.

Also fixed while in there: the input's `step` was `0.0001` for ICP, a UI-only granularity the
canister never asked for — it made an exact balance unrepresentable in its own input box. It is
`0.00000001` now, the full precision of a smallest unit.

#### The rest of the audit, for the record

Every other place the app states a limit, a minimum, a maximum, a fee or a timeout was checked
against the constant the canister enforces. The ones that AGREE are worth listing, because a census
that only reports failures cannot be told from one that was not run:

| surface | UI states | canister enforces | verdict |
|---|---|---|---|
| Withdraw ICP minimum | 0.001 ICP | `ICP_MIN_WITHDRAWAL_AMOUNT` 100,000 e8s | agrees |
| Withdraw fee, both currencies | 10 sats / 0.0001 ICP | `CKBTC_TRANSFER_FEE` 10, `ICP_TRANSFER_FEE` 10,000 | agrees |
| Deposit minimum, both currencies | 1,000 sats / 0.0002 ICP | `deposit()` 1,000 / 20,000 | agrees |
| Deposit `InsufficientFunds` fee text | 10 sats / 0.0001 ICP | same two constants | agrees |
| Lobby blinds, buy-in range, seat count | read from each table's own `config` at runtime | the same struct | cannot disagree: no second copy |
| Lobby clock `{action_timeout_secs}s + {time_bank_secs}s` | from `config` | same | cannot disagree |
| `PokerTable` clock fallbacks `?? 30` / `?? 60` | 30 s time bank, 60 s action | `DEFAULT_TIME_BANK_SECS` 30, `DEFAULT_ACTION_TIMEOUT_SECS` 60 | agrees |

One disagreement of the same *kind* is out of this pass's reach, and is recorded rather than fixed
because `src/table_canister/src/lib.rs` is not this pass's file:

```rust
// src/table_canister/src/lib.rs:2464
// Standard poker: min buy-in should be at least 20 big blinds
if config.min_buy_in < config.big_blind * 10 {
    return Err("min_buy_in should be at least 10 big blinds".to_string());
}
```

The comment says **20 big blinds**, the code enforces **10**, and the error message says 10. It is
not player-facing today — `validate_config` is reached only from table creation and the app has no
create-table screen — so nobody can be lied to by it yet. Whoever owns that file should pick a
number and make the comment say it.


<a id="t-30"></a>
### T-30 — medium — the deposit form's "you have enough" test understated the requirement by two fees — STATUS: FIXED (wave 4)

```
DepositModal.svelte:399  return bal !== null && bal > Number(minDeposit);
DepositModal.svelte:634  const hasEnoughBalance = $derived(effectiveWalletBalance !== null &&
                                                            effectiveWalletBalance > Number(minDeposit));
```

An ICRC-2 deposit costs the depositor **two** ledger fees, both charged to their account: one for
`icrc2_approve` and one for the canister's `icrc2_transfer_from`. `deposit()`'s floor is 1,000 sats
/ 20,000 e8s, so the real requirement to make the minimum deposit is 1,020 sats / 0.0004 ICP. The
test compared against the floor alone, so a wallet holding 1,005 sats was shown the deposit form
and every amount it could enter would have been refused by the ledger. (`setMaxAmount` in the same
file already held back `20 / 20000` — 2× the fee — so the correct number was known one function
away and not used.)

The test is now `balance >= MIN_DEPOSIT + 2 × TRANSFER_FEE`, both duplicate copies of it are one
`$derived`, and the modal states the wallet figure as well as the deposit figure:
`Minimum deposit: 1,000 sats (network fee 10 sats, charged twice by the ledger, so you need
1,020 sats in your wallet to deposit the minimum)`.

<a id="d-05"></a>
### D-05 — low — the only two limits in the app that nothing in this repository can verify — STATUS: BY-DESIGN (wave 4)

The native-BTC deposit path states a **10,000 sat** minimum and a **~2,000 sat** ckBTC cost, in
four places in `DepositModal.svelte`. Neither figure is enforced by anything in this tree: the
address belongs to the ckBTC **minter**, an external canister, and `src/table_canister` has no
constant for either. `ui_limits.rs` therefore cannot check them, and this entry exists so that
absence is recorded rather than assumed.

Reduced but not closed in this pass: the four mentions are now interpolated from two named
constants (`BTC_NATIVE_MIN_SATS`, `BTC_NATIVE_MINTER_FEE_SATS`) carrying a comment that says they
are the minter's and not ours, so they can no longer disagree with *each other* — which is the shape
T-26 took. What remains open is that they can still disagree with the minter. Closing it needs a
live query of the minter's `get_minter_info` / retrieval-fee endpoints and a note on screen saying
when it was last read; that is a network call this modal does not make today.

<a id="t-27"></a>
### T-27 — low — the equity method line disappears exactly when the equity becomes a verdict — STATUS: FIXED (wave 5)

**Status FIXED 2026-08-05.** The line is now a Svelte `{#snippet}` rendered into whichever
readout is on screen — the pot display or the winner banner — so one string cannot be dropped by
a branch again. Inside the banner it hangs OFF the pill rather than growing it (the banner's
height is tuned to a 166 px slot in portrait), on the side facing away from the board: above the
banner in landscape, below it in portrait, which is where each orientation has felt.

Measured at the showdown after the fix, on the deployed build: the scene notes now read
`Seat 2 wins 24.00 ICP Straight Complete Equity · exact · 1 runout` at desktop and
`You won 24.00 ICP Pair Complete Equity · exact · 1 runout` at mobile — the method is in the
same frame as the two solid `100.00% / 0.00%` badges, at both viewports, and the pixel gate
reports it unoccluded there.

**The original report follows.**

`.equity-method` — `EQUITY · EXACT · 1 RUNOUT`, or the full `vs N random · MONTE CARLO · 200,000
TRIALS` statement — lives inside `.pot-display`, which is replaced by the winner banner once the pot
is zero. Measured at the showdown on both viewports: `.equity-method` is **null** while two solid
`100.00% / 0.00%` badges are on screen. The method survives only in a `title` attribute, which a
phone cannot open. The honesty framing the feature was built on is missing from the one frame that
is photographed.

<a id="h-30"></a>
### H-30 — medium — the shuffle verdict is below the fold at both viewports — STATUS: OPEN

`.headline` reaches `4 of 4` rungs and a green verdict at both viewports — the trust pass's central
claim, and I reproduced it. But `headlineInViewport: false` on **desktop as well as mobile**. On
mobile it is structural rather than a matter of scroll position: the panel sits inside
`.proof-sidebar` with `clientHeight 918` and `scrollHeight 2708` while `document.scrollHeight` is
only 1293, so the verdict at `y = 1717` cannot be reached by scrolling the page at all — only by
scrolling a nested container a player has no reason to know exists. The scene passes because it
asserts the DOM, not the pixels.

<a id="h-31"></a>
### H-31 — medium — the hand-history scene never opens a hand — STATUS: FIXED (wave 4)

`tools/shots/scenarios/handhistory.mjs` asserts the LIST and stops. The replayer behind
`.hand-row` is where [T-21](#t-21) lived undisturbed for a whole wave, where the action-log A/B
against PokerNow is lost, and where the only per-card verification surface in the corpus lives. It
has never been photographed and nothing about it is asserted. One `page.click('.hand-row')` and a
handful of assertions closes it.

**Closed by `tools/shots/scenarios/handreplay.mjs`** — the full account, the assertion table and the
five mutations that prove each assertion fires are in [H-31 (fixed)](#h-31-fixed) at the end of this
file. It took rather more than a handful of assertions: opening the replayer also uncovered
[H-32](#h-32) (the two fields the log is made of were unreachable through the client's Candid
declaration), [H-33](#h-33), [H-35](#h-35) and [H-36](#h-36).

<a id="d-04"></a>
### D-04 — medium — the corpus does contain a real mobile lobby capture — STATUS: FIXED (wave 4)

`docs/DESIGN-BAR.md` §9.4.1 said "There is no real mobile lobby capture in the corpus", and set no
mobile first-row bar on that premise — for exactly the metric ClearDeck fails worst.

`$SCRATCH/reference/pokerstars/web-ps-gipsy-2.png` is indexed `client=pokerstars`,
`real_gameplay=true`, `capture_type: "third-party review (real client screenshots)"`, notes
"Real PokerStars mobile client: lobby list, Spin&Go table, store". Its left panel is a complete phone
screen: app top bar, tournament lobby list, bottom tab bar. I opened it and measured it with a row
luminance-step detector: screen rows **0..567**, first tournament card at **y ≈ 116 = 20.4%**, row
pitch **~85 px**, **4 rows fully visible** and a 5th clipped by the tab bar.

The doc missed it by querying `INDEX.json` for `scene == 'lobby-mobile'`; the file is filed
`scene == 'mobile-portrait'`. **BAR 31** in `DESIGN-BAR.md` now exists and cites it. ClearDeck's
mobile lobby measures a first row at **y = 543 = 64.3%** with **1** row fully visible.

---

<a id="h-31-fixed"></a>
### H-31 — medium — the hand replayer is now opened, walked and asserted — STATUS: FIXED (wave 4)

**Where** `tools/shots/scenarios/handreplay.mjs` (new scene, registered in `scenarios/index.mjs`),
`tools/shots/lib/protected-notices.mjs` (new), `tools/shots/lib/hand-record-wire.mjs` (new).

The replayer behind `.hand-row` had never been clicked by anything in this repository. Nothing about
it was asserted, no PNG of it existed, and every number it renders was outside the token census —
the census can only see what a scene puts on screen.

**What the scene asserts, on the rendered page, in the only legal configuration**

| # | assertion | measured this run |
|---|---|---|
| 1 | every street stop clicked; board grows | `Pre-flop 0 · Flop 3 · Turn 4 · River 5 · Showdown 5`, every card's face equal to `get_hand_history(n).community_cards`, **0 crosses / 5 ticks** |
| 2 | every log line against the hand record | **10 of 10** lines: street, actor seat, verb, class and amount |
| 3 | the log's own arithmetic | blinds + every recorded amount = **144,000,000 e8s** = what the winners were paid, and the screen says so |
| 4 | money figures | **15** compared with the canister (13 in the replayer, blinds against `get_table_view().config`), 0 mismatches |
| 5 | the fairness claims | banner tone `good`; headline carries no ordering claim; three retired over-claim strings absent; `Not proven` present; caveat carries it |
| 6 | the ordering, witnessed | `data-witness="matched"`; the sighting store holds the hand's own commitment, read at `PreFlop` with 0 board cards; paste-to-compare `good` / `bad` / `good` |
| 7 | the four protected notices | **5 of 5 on screen and unoccluded**, hit-tested |
| 8 | the token census | 191 (desktop) / 166 (mobile) numeric tokens, **0 unaccounted for** |
| 9 | the replayer opened on a hand STILL BEING PLAYED refuses to sum it | `data-audit="not-checkable"`, *"this hand has not been settled yet, so there is no pot for the amounts to be checked against"* |

Row 9 is a defect this pass would otherwise have introduced. The table writes a hand's record when
the hand **starts**, so the replayer can be opened on a record with actions and no winners; the first
version of the audit summed the blinds against a pot of zero and announced *"the amounts come to
0.03 ICP but the table paid out 0.00 ICP. One of the two is wrong"* about a hand that was simply not
finished. `potAudit` now refuses on an unsettled record, and the scene walks that path deliberately —
it opens the live hand mid-staging (switching the list filter first, because "My hands" cannot match a
hand whose record has no showdown players yet) and fails if the audit does anything but refuse.

**Captures** `artifacts/screens/<sha>/handreplay-{desktop,mobile}.png` plus a second frame per
viewport at `motion/handreplay/frames-{desktop,mobile}/frame-000.png`, because one frame cannot hold
the record: the replayer's content measures **1119 px** (desktop) and **2480 px** (mobile) inside a
scroller **689 px / 618 px** tall. `checks.fold` in the manifest names, per element, whether it falls
inside the dialog — the measurement [H-30](#h-30) says nobody was making.

**Proved by breaking it.** Four mutations, each caught, transcripts in `$SCRATCH`:

* reverting [H-32](#h-32) (the Candid declaration) → **14 problems**, each naming the truth:
  `log line 3 (action 1 (Call)) shows NO amount, but the canister recorded 1000000 e8s`,
  `... is under street "Street not recorded", not "Pre-flop"`, plus the audit line flipping to
  `unbalanced` and the census finding the wrong sum (`0.73` where the pot is `1.44`) unasserted.
* restoring the [T-21](#t-21) headline → `the headline verdict claims an ordering: "These cards were
  fixed before the hand was played, before any card was dealt."` and all three over-claim strings
  found in the modal.
* restoring the `committed before the deal` label → `a proof label claims an ordering`.
* `display: none` on the in-dialog notices → **0 of 5** protected phrases on screen, on both scenes.
* not recording the mid-hand sighting → `the witness panel reads "none", not "matched"`.

**One thing the scene must not be trusted about**: it reads its chain truth through
`lib/hand-record-wire.mjs`, not through `src/declarations`. The first version shared the app's
declaration and, when H-32 was reverted, both sides of every street and amount comparison went blind
together: the scene went red, but reported `not "null"` instead of the amount the canister had
recorded. An assertion has to know the truth, not merely differ from the screen.

---

<a id="h-32"></a>
### H-32 — high — the Candid declaration dropped the two fields the action log is made of — STATUS: FIXED (wave 4)

**Where** `src/declarations/table_1/table_1.did.js` (and its `.d.ts`), generated from
`src/table_canister/table_canister.did`. Root cause is [E-08](#e-08), a different owner's file.

`ActionRecord` in `src/table_canister/src/lib.rs` has carried, since before wave 1:

```rust
pub struct ActionRecord {
    pub seat: u8,
    pub action: PlayerAction,
    pub timestamp: u64,
    pub phase: String,   // "preflop" | "flop" | "turn" | "river"
    pub amount: u64,     // the actual amount, including Call and AllIn
}
```

`apply_player_action` fills both for **every** action it records. The client's declaration named only
`{ action; seat; timestamp }`, and Candid record subtyping means a decoder silently **drops** fields
it does not declare. So the two fields the log needed were on the wire and unreachable:

* `PlayerAction::Call` and `PlayerAction::AllIn` carry **no payload**, so the only place a call's or
  an all-in's amount exists is `ActionRecord.amount`. That is why the replayer's log carried an
  amount on **none** of its lines in the wave-4 A/B against PokerNow, which carries one on 6 of 11.
* the street was equally unreachable, which is why the log printed a note saying the table "does not
  tag [an action] with the street it happened on". It does.

**Proved on this replica** before anything was changed, with a throwaway actor built from a wide IDL:
`get_hand_history(1)` on `table_2` returned 8 actions, every one carrying `phase` and `amount`
(`{Call:null} seat 1 phase "preflop" amount 5000000`, …).

**Fix** the two fields added to the declaration, with the reason in a comment beside them. Adding
fields a decoder can already see on the wire cannot break an older canister — the encoder is the Rust
struct. `table_2`/`table_3`'s copies of the declaration are unused (the app and the harness both load
`table_1`'s) and are left for whoever regenerates them with the `.did`.

---

<a id="h-33"></a>
### H-33 — medium — a staging wait that could not fail — STATUS: FIXED (wave 4)

**Where** `tools/shots/scenarios/handhistory.mjs`.

```js
() => document.querySelectorAll('.community-cards .card').length >= 5
```

`PokerTable.svelte` renders `{#each Array(5) as _, i}<Card card={communityCards[i] ?? null} />`
unconditionally, and an undealt slot is still a `.card` — it only carries `.empty`. So this predicate
was true the instant the board frame mounted, dealt cards or not, and the comment above it described
a signal it was not reading. It stood in for "the app has seen the finished hand", which is the one
piece of state the scene depends on.

`:not(.empty):not(.face-down)` counts cards that are actually face up. `handreplay.mjs` uses the same
form to wait for an EMPTY board (0 face-up cards) as the signal that the app has seen a NEW hand,
which the vacuous version could not have expressed at all.

---

<a id="h-34"></a>
### H-34 — medium — the pixel gate has no notion of a dialog, so every modal scene fails it — STATUS: FIXED (wave 5)

**Status FIXED 2026-08-05 by the gate's owner**, exactly as prescribed below. `lib/occlusion.mjs`
now identifies the **overlay layer** an element belongs to — a `position: fixed` ancestor (or self)
covering ≥40% of the viewport, or anything carrying `role="dialog"` / `aria-modal="true"` — and an
occluder inside an overlay that covers a figure **on the page** is reported as `behind-an-overlay`
with its measured fraction, not gated. The test is deliberately one-directional: a figure *inside* an
overlay is gated normally, so a dialog that covers its own numbers still fails, and so does a second
overlay covering the first one's. `test-occlusion.mjs` pins both halves ("a dialog covering the page
behind it does not fail the scene", "a figure INSIDE a dialog, covered by the dialog's own chrome,
still fails").

Measured after the fix, with no `SHOTS_OCCLUSION=report` anywhere: `handhistory` and `handreplay` are
**verified at both viewports** with `0 occluded`, and the dialogs' effect on the page is still in the
artifact as a number — 37 figures at `handhistory` desktop, 36 at `handreplay` mobile, 186 across the
whole sweep, each with the fraction of its ink the dialog takes. That list is also the corroboration
for [T-31](#t-31)/[H-36](#h-36): at `deposit` desktop it names `p.banner-info` at **79.9%** and
`p.banner-warning` among the figures the backdrop suppresses.

The original report follows, because the reasoning in it is what the fix implements.

**Where** `tools/shots/lib/occlusion.mjs` (a different owner's file, landed during this pass).

The gate is right about what it measures and the two defects it was built for are real. But its
target set is "money, equity and cards", its occluder set is "anything painted on top", and a modal
is a thing deliberately painted on top of a table. So:

| scene | viewport | findings | occluders |
|---|---|---|---|
| `handhistory` (pre-existing) | desktop | 48 | `.hand-history-modal` 37, `.modal-content` 11 |
| `handreplay` | desktop | 60 | `.hand-history-modal` 37, `.modal-content.wide` 23 |

**Every** occluder in both scenes is the dialog or its overlay; **zero** are caused by anything added
in this pass (checked: no finding names `.legal`, and no protected-notice carrier is among the
occluded targets). `handhistory` is a scene nobody touched in this respect, and it fails identically,
which is what makes this the gate's gap rather than the component's.

The gate's own header says a gate that cries wolf gets switched off. The fix belongs in
`occlusion.mjs`: an occluder that is, or is inside, an open dialog overlay is intentional and should
be reported rather than gated (or gated only against things inside the same dialog). Until then the
two hand-history scenes can only reach a canonical filename with `SHOTS_OCCLUSION=report`, which the
manifest labels `REPORT ONLY (SHOTS_OCCLUSION=report) — NOT GATING` in every row it appears in.

---

<a id="h-35"></a>
### H-35 — medium — three claims T-21 left behind in the same modal — STATUS: FIXED (wave 4)

[T-21](#t-21) demoted the replayer's green headline. Three other claims in the same component still
said the retired thing, or said more than was counted:

1. **`.proof-label`: "committed before the deal".** The label on the commitment row asserted, as
   fact, the exact ordering the banner two panels above it lists under `Not proven`. Now
   "commitment in this hand's record"; the seed row is "seed, published with the finished hand".
2. **The downloadable audit file: `commitment_published_before_deal`.** A field NAME is a claim, and
   this one shipped in a JSON file the player keeps. Now `commitment_in_the_hand_record`, and the
   file carries an explicit `not_proven` string plus, when the browser has one, its own mid-hand
   sighting of the commitment.
3. **`.list-foot`: "Every hand above was re-derived in this browser from its own revealed seed".**
   False whenever a hand is in play: that hand is listed with its seed sealed and cannot have been
   re-derived. Now counted — "N of M hand(s) above were re-derived …" — from the same
   `verification.ok` flag the per-row badge uses.

All three are gated by `handreplay.mjs`: `.proof-label` and `.proof-banner strong` must not contain
the word "before", the modal must contain "Not proven", and three historical over-claim strings must
be absent from it.

**What replaced the claim, rather than just removing it.** The commitment is on screen from the
moment cards are dealt and the seed is not published until the hand ends, so a player who reads the
commitment mid-hand and compares it afterwards has established the ordering themselves, with no
canister clock in the argument. `$lib/commitment-witness.js` does the reading automatically — one
sighting per (table, hand), never overwritten, never taken once a seed exists — and the replayer
states exactly what it proves: *"the deck was already fixed at the moment you looked: before every
card dealt after it, and before every action taken after it."* A paste box compares a commitment the
player kept elsewhere. Both verdicts are asserted by the scene.

---

<a id="h-36"></a>
### H-36 — high — the four protected notices are behind every dialog's scrim, and wave 4's check could not see it — STATUS: FIXED (wave 9)

**Where** `.alpha-warning-banner` / `.footer-disclaimer` versus every full-screen dialog in the app.
**Fixed for the hand-history dialog. Open for `DepositModal`, `WithdrawModal` and any future
overlay** — those are other owners' files.

[T-20](#t-20) fixed a portrait table showing 0 of 4 notices, and wave 4 re-measured: banner
`y 0..268`, "fully in the viewport", "**and behind the open Deposit modal**". That last clause is a
GEOMETRY result, and geometry cannot see a black sheet. Every dialog here is
`position: fixed; inset: 0` over `rgba(0,0,0,0.72)` with a 4 px blur.

**Measured.** `tools/shots/lib/protected-notices.mjs` finds the deepest element carrying each
protected phrase and hit-tests its own pixels with `elementFromPoint`. With the hand-history dialog
open and its in-dialog copy of the notices suppressed (`display: none`, the wave-4 failure mode
exactly): **0 of 5 phrases on screen, 4 carriers in the DOM**, on both scenes and both viewports.
`make hygiene` stays green throughout, correctly — it greps the source.

**Fix, for this dialog.** The four notices ride inside the dialog, verbatim, pinned outside the
scrolling region (`flex-shrink: 0`) so they cannot be scrolled away, on the list and the replayer, at
every viewport, together with the no-rake property. Nothing anywhere else is weakened; this is an
additional copy, and more prominent is always allowed. Both hand-history scenes now assert **5 of 5
on screen and unoccluded**, and both go red if that stops being true.

**Recommended next**: the same strip (or the same probe) for `DepositModal` and `WithdrawModal`. The
deposit modal is the screen a player commits real ICP from, and it is the exact configuration wave 4
named.

---

<a id="h-37"></a>
### H-37 — high — BUILT IN WAVE 5 — the harness could not see what covers what — STATUS: FIXED (wave 5)

**The gap.** Every gate in this repository reads `textContent`: `chain-agreement.mjs` asks whether
the string equals the canister's number, `token-census.mjs` asks whether every string is accounted
for, the protected-notice probes ask whether a phrase is present and hit-testable. None of them can
answer the only question a player's eye asks — **is the figure on screen, or is something on top of
it**. [T-22](#t-22) and [T-23](#t-23) were both photographed by this harness and filed as VERIFIED,
with the canonical filename, because the text was right.

**What was built.** `tools/shots/lib/occlusion.mjs`, run centrally in `run.mjs` so no scene can
forget it, plus `lib/png.mjs` (a zlib-only PNG codec, because the pixel gate must not be the one gate
that can fail to install) and `test-occlusion.mjs` (15 offline cases). It enumerates every money,
equity or card figure on screen and requires that nothing paints over it. Three stages, and the last
one decides:

1. **geometry** — client-rect intersection inside the visible viewport;
2. **effective paint order** — CSS 2.1 Appendix E simulated over the computed styles: stacking
   contexts from `transform` / `opacity` / `filter` / `contain` / `container-type` / `position:
   fixed|sticky` / positioned-with-a-z-index / flex-grid items with one, the *pseudo* contexts that
   positioned `z-index: auto` elements form (whose positioned descendants escape to the parent
   context), and the tree-order layers. One integer per element, so "is above" is one comparison that
   is right across contexts;
3. **occlusion evidence** — `elementFromPoint` on a 5×3 grid, and a **four-shot pixel differential**
   per (figure, occluder): `A` both visible, `B` occluder hidden, `C` figure hidden, `D` both hidden,
   clipped to the figure. The figure paints ink where hiding it changes the pixel (`|B−D|`) and is
   COVERED where the occluder suppresses ≥75% of that contribution (`|A−C|`). Hiding is
   `visibility: hidden`, which paints nothing and moves nothing.

**Why the pixel differential and not the hit test.** Hit testing cannot see an occluder with
`pointer-events: none` — `.board-cluster` is exactly that in portrait — and on one measured hand
`elementFromPoint` at the centre of an equity badge answered *the badge* while 46.8% of it was
painted over. The differential also cannot be fooled by a rect intersection that covers only padding.

**Thresholds.** 2% of a figure's own ink for money, equity and a card's rank or pip (one glyph of a
seven-glyph badge is ~14%, so no covered digit can pass, while a padding-only overlap measures
0.0%); 20% for a card's blank face, because its rank and pip are gated at 2% in their own right and
the hero's own bet disc touching the hero's own card measures 2.8% — failing that would be a false
red on the first scene anyone ran.

**Measured false-positive rate, whole sweep** (22 shots, 542 figures, 2,895 pixel-tested pairs):

| gate | flags raised | real |
|---|---|---|
| rects intersect **and** raw z-index is higher (the naive gate) | **3,836** | 0 |
| rects intersect **and** effective paint order puts it above | **1,487** | 0 |
| ...and the pixel differential confirms it (this gate) | **0** | 0 |

with 186 further overlaps classified `behind-an-overlay` (a dialog covering the page it is over,
see [H-34](#h-34)), 2 figures 3 px tall at the viewport's bottom edge recorded as not measurable
rather than judged, and **0 paint-order model disagreements** — no case where the pixels showed
coverage the paint model said was impossible.

**Three defects were found in the gate itself while building it, and each is now pinned by a case in
`test-occlusion.mjs`:**

1. **A figure that is also an occluder got its probe id overwritten**, so hiding "the figure" hid
   nothing, all four screenshots came back identical, and it was reported as having no ink and no
   occlusion. The award chip is both a money figure and the occluder of a card — that is T-23
   exactly — so the first draft of this gate produced a **false GREEN on the very defect it was
   built for**. Ids are interned per element now.
2. **The coverage test was an absolute colour difference.** A translucent veil over a figure whose
   own background is close to the page's pushed `|A−C|` under a fixed tolerance and the whole figure
   was reported covered while every digit was still readable. It is a suppression RATIO now, which is
   scale-free.
3. **`page.screenshot({ clip })` trims the clip against the VIEWPORT, not the document.** The first
   draft added `scrollX/scrollY`; identical while a page happens to be at scroll zero, it threw on
   `deposit` at mobile (where the open modal leaves the page scrolled 133 px) — and the dangerous
   version of that mistake is the one that does not throw but measures a **different rectangle** with
   total confidence. Demonstrated directly (`$SCRATCH/pixelgate/clipsem.mjs`: viewport coordinates
   return the marked pixels, document coordinates throw). Every figure's `inkPixels` is now asserted
   per figure, so a crop that does not contain its figure is recorded instead of trusted.

**Reproduce.**

```bash
node tools/shots/test-occlusion.mjs                 # 15 cases, no replica, no app build
node tools/shots/run.mjs                            # the gate runs on every scene, both viewports
SHOTS_OCCLUSION=report node tools/shots/run.mjs     # measure without gating (labelled in the manifest)
```

**Still outside the gate**, stated so nobody mistakes silence for a pass: an occluding
`::before`/`::after` drawn by an ANCESTOR of a figure cannot be hidden independently of the figure
itself; a translucent overlay that leaves >25% of the figure's contribution is reported as
`alteredInkFraction` and does not fail; contrast is not measured, so a legible figure and an
illegible one of the same colour are the same to this gate; and a figure scrolled out of the
viewport is not judged at all.

<a id="h-38"></a>
### H-38 — medium — the harness's three self-checks are named by no make target — STATUS: FIXED (wave 7)

`tools/shots/test-census.mjs`, `test-money.mjs` and the new `test-occlusion.mjs` are the only
things that check the *verifiers* — the census that decides whether a green scene means anything,
the money parser, and the pixel gate. `grep -n 'test-census\|test-money\|test-occlusion'
scripts/dev.sh Makefile .github/workflows/*.yml` returns **nothing**. This is the same shape as
[H-17](#h-17) (the fund-theft reproducer that no target named for a whole wave) and it ends the same
way: a self-check nobody runs is a self-check that has already stopped working.

They need no replica, no canister and no app build, and together they take about 15 seconds.
`npm run selftest` in `tools/shots` now runs all three (that file is inside the harness), but the
one-line fix belongs to `scripts/dev.sh`, which is a different owner's file:

```sh
# in cmd_test, beside the workspace tests
( cd tools/shots && npm run selftest )
```

---

## Found by the wave-5 lobby pass

Everything in this section was found by measuring the **rendered** lobby — `getBoundingClientRect()`
and `elementFromPoint()` in the browser the screenshot harness drives, against the real local
canisters, at ten viewport widths from 390 to 1920 — and by reading the lobby canister's own
registry with `icp canister call ... --query -e local`. Scratch harness:
`$SCRATCH/w5/{measure,sweep,probe,modal-probe}.mjs`; raw output `$SCRATCH/w5/{measure-*,sweep}.json`.
Geometry results and the reference comparison are in
**[DESIGN-BAR.md §9.4.2 – §9.4.2c](DESIGN-BAR.md#942-what-cleardeck-did-and-what-it-does-now)**.

> **HISTORICAL SNAPSHOT — the status column below is NOT maintained.** It is the queue as it was written in this wave. The only true statuses are in [THE REGISTER](#the-register).

| # | sev | status | where | one line |
|---|---|---|---|---|
| [L-01](#l-01) | **high** | **FIXED IN THIS PASS** | `Lobby.svelte` `.list-pane` / `.tables-list` | every row's `Sit` / `View` / `Watch` control was **clipped** — 10.5 px of it, arrow included — at 1440×900, and at three more width ranges besides. A control a player clicks, cut off by `overflow: hidden`, at the project's own reference viewport |
| [L-02](#l-02) | medium | **FIXED IN THIS PASS** | `HowItWorks.svelte` `.modal-content` | the How-it-works dialog opened **behind the disclaimer banner**: its title row and its close `×` were unreachable by mouse at 1440×900. Only the keyboard path worked |
| [L-03](#l-03) | **high** | open — **other owner** (`+page.svelte`) | `.alpha-warning-banner` + `header` in portrait | the phone lobby cannot reach the reference band from `Lobby.svelte` at all: with the lobby's own furniture at **zero** the first card is still at **45.9%**. The 28 px needed are chrome, and the mechanism to release them already ships one condition away |
| [L-04](#l-04) | **high** | open — needs 2 admin calls **and** 1 lobby method that does not exist | lobby canister registry | the lobby's registered *names* quote blinds the table contracts do not charge (10× and 5× wrong), and its registered *configs* disagree with the contracts on four fields each. **This, and only this, is why both lobby scenes are red.** Every price the client computes is the contract's |
| [L-05](#l-05) | low | open — needs 1 admin call | lobby canister registry, `scripts/dev.sh up_wire` | [T-05](#t-05) determined: registering `btc_table_1` is **not** a frontend fix. The exact call, plus the 35 px it costs the phone layout |
| [L-06](#l-06) | medium | open — **other owner** (`tools/shots/lib/occlusion.mjs`) | `artifacts/screens/<sha>/manifest.json` | the new occlusion pixel gate writes **60 KB per shot** into the manifest; one run's manifest is **5.58 MB**, which trips `make hygiene`'s own 4 MiB untracked-payload check. A green harness now makes a red hygiene |

<a id="l-01"></a>
### L-01 — high — every row's action control was clipped, at four separate width ranges — STATUS: FIXED (wave 5)

**Status: executed.** Measured on the rendered page, `dist` built from `fe72d46`, before any change
this pass.

At 1440×900 the lobby's `<table>` reported a **min-content width of 934.5 px inside a 908 px pane**.
`.list-pane { overflow: hidden }` clipped the difference, and the last column is the action column:

```
.list-pane        left 80.0   right 990.0   (clientWidth 908, scrollWidth 935)
.go "View"        left 953.1  right 999.5   -> 10.5 px outside the pane
.go "Watch"       left 945.6  right 999.5   -> 10.5 px outside the pane
```

Seven columns of `white-space: nowrap` content plus 16 px of cell padding a side is what set the
floor. **No gate in the repository could see it**: every lobby assertion reads `textContent`, and
the text was all present — it simply was not painted.

It was not one breakpoint. Which columns fit is a question about the **pane's** width, and every
rule deciding it was written against the **viewport's** — so the pane silently narrowed by 320 or
352 px whenever the preview appeared. Swept at 13 widths, the clipping ranges were:

| range | columns shown | pane | table min-content | clipped by |
|---|---|---|---|---|
| 761–848 | 6 (no Hands) | 719–806 | 807 | up to 88 px |
| 1001–1064 | 5 (no Hands, no Buy-in) | 623–686 | 687 | up to 66 px |
| 1081–1184 | 6 (no Hands) | 703–806 | 807 | up to 104 px |
| 1241–1288 | 7 | 831–878 | 879 | up to 48 px |

**Fix.** Root cause, not breakpoints: `.list-pane { container-type: inline-size }` and two
`@container` rules at the measured floors (879 px of pane for seven columns, 807 for six, 687 for
five), cell padding 16 → 12 px, and the preview drops below the list at 1080 rather than 1000 so the
pane is never narrower than the five-column floor. Ten widths from 390 to 1920 now report **zero**
clipped containers (`.bg-effects`, a fixed decorative layer in `+page.svelte`, excluded — no
content). No column was removed from the 1440 grid and no field was dropped from the row.

<a id="l-02"></a>
### L-02 — medium — the How-it-works dialog opened behind the app chrome — STATUS: FIXED (wave 5)

**Status: executed.** `.alpha-warning-banner` is `z-index: 100` on a child of `.app`; the dialog is
`z-index: 1000` but renders inside `<main>`, which is its own stacking context, so the banner and
the header paint over it however high its z-index goes. Centred at `top: 50%`, the dialog box began
at **y = 67.5 under 243 px of chrome** at 1440×900:

```
.modal-content   top 67.5   bottom 832.5
.close-btn       top 88.5   elementFromPoint(centre) -> p.banner-warning   (unclickable)
```

Escape and the autofocused close button still worked, which is why nothing noticed.

**Fix, and why not the obvious one.** Raising the z-index would have put the dialog over the
unaudited-alpha disclaimer, the 18+ notice, the jurisdiction warning and the no-house statement,
which is exactly what [T-31](#t-31)/[H-36](#h-36) forbid. So the dialog measures the chrome instead
(`.alpha-warning-banner` / `header`, re-read on resize **and scroll**, since the chrome is in the
flow and its viewport-relative bottom moves) and opens under it. Measured after: dialog top **253**
at 1440×900 and **398** at 390×844, close button hit-testing to itself at both, title on screen at
both, and 4 of 4 protected phrases plus the no-house property still on screen and unoccluded with
the dialog open. The four phrases are additionally restated **inside** the dialog, pinned outside its
scrolling region, so the guarantee no longer depends on another component's stacking order.

<a id="l-03"></a>
### L-03 — high — the phone lobby's remaining gap is 28 px of chrome, and it is not the lobby's to spend — STATUS: OPEN

**Status: executed.** [DESIGN-BAR BAR 31](DESIGN-BAR.md#6-the-bars-cleardeck-has-to-clear) wants the first card inside
the top 45% of a 390×844 screen; the reference band is 19.9%–42.6%. Measured this pass:

```
banner 268.0 + header 119.5   = 387.5 px = 45.9% of 844   <- before Lobby.svelte paints a pixel
lobby furniture               =  96.8 px                  (was 155.0; BAR 26 allows 160)
first card                    = 484.3 px = 57.4%          2 of 3 cards fully visible
```

Drive the lobby's furniture to **zero** and the first card still lands at **45.9%**, 3.3 points
outside the band. **28.0 px** has to come out of the 387.5 px above the lobby, and all of it belongs
to `+page.svelte` / `index.scss`.

**The mechanism already exists in `+page.svelte`.** Wave 5's concurrent pass added `.banner-strip`,
a compact strip carrying the protected phrases verbatim, one tap from the full text, and scoped it
`class:on-table={view === 'table'}` with the note "the lobby in portrait still gets the full banner
in the flow, because on the lobby nothing is competing for the space". On the lobby something is:
the first-row bar. Applying that same treatment to the lobby (simulated in the page by adding
`on-table` to `.alpha-warning-banner` and `compact` to `header`; **`+page.svelte` was not edited**):

| | banner | header | chrome | first card | cards fully visible | notices on screen |
|---|---|---|---|---|---|---|
| ships today | 268.0 | 119.5 | 387.5 | **57.4%** | 2 of 3 | 4 of 4 |
| with the strip | 59.6 | 42.0 | 101.6 | **23.5%** | **3 of 3** | **4 of 4** |

**Exact change**, for the owner of `+page.svelte`: make the collapsed portrait presentation
conditional on portrait alone rather than on `view === 'table'` — i.e. the class that drives it stops
carrying the view test, and `header.compact` follows it. Nothing in the wording, the phrase list or
`make hygiene` changes; the strip already quotes all four notices verbatim, and the measurement above
confirms 4 of 4 on screen and unoccluded in that configuration.

<a id="l-04"></a>
### L-04 — high — the lobby registry quotes prices its own contracts do not charge, and no method can fix half of it — STATUS: FIXED (wave 14, LOCAL ONLY)

> **FIXED 2026-08-09 on the local replica. Mainnet is untouched and still needs the same one call.**
>
> The NAME half was closed in wave 13 by deleting the price from every registered name. The CONFIG
> half needed a method that did not exist, and this is it — deliberately **not** the
> `update_table_config(id, config)` this entry sketched:
>
> ```candid
> refresh_table_config      : (nat64) -> (Result);      // admin only
> refresh_all_table_configs : ()      -> (Result_2);    // admin only
> ```
>
> They call `get_table_view()` on the table and COPY its config. An admin setter can be handed a
> wrong number, which is the defect it would be fixing; a copy cannot. The only way for the registry
> to be wrong now is for the contract to be wrong, and that is not a registry defect.
>
> ```text
> $ icp canister call lobby refresh_all_table_configs '()' -e local --identity cd-local-deployer
> (variant { Ok = vec { 2 : nat64; 1 : nat64; 3 : nat64 } })
>
> lobby id 2 "6-Max"    5_000_000/10_000_000   buyin 1_000_000_000-5_000_000_000   seats 6
> lobby id 1 "Heads Up" 1_000_000/2_000_000    buyin   200_000_000-1_000_000_000   seats 2
> lobby id 3 "9-Max"   10_000_000/20_000_000   buyin 2_000_000_000-10_000_000_000  seats 9
> ```
>
> which is what the three contracts charge, field for field.
>
> **CORRECTION to this entry: `up_wire()` is no longer failing silently.** It records that the local
> lobby's admin was the machine's default identity and that `set_admin(cd-local-deployer)` was being
> refused. `lobby.get_admin()` now answers `sg3sw-…-6qe`, which *is* `cd-local-deployer`, so the two
> calls this entry could not issue are issuable by the project's own identity.
>
> **Gate:** `./scripts/check-deployed-config.sh` now reads `lobby.get_tables()` and diffs every
> registered row against its own contract, plus a rule that a registered NAME may not quote a price
> at all. Proved red by putting the drift back — name to `"6-Max - 0.05/0.10"`, and the table_2
> contract's clock moved 45 -> 60:
>
> ```text
> ✗ lobby row 2 ("6-Max - 0.05/0.10") -> 4fbx2-kt777-77775-aaabq-cai
>       small_blind: lobby advertises 1000000, the contract charges 5000000
>       big_blind: lobby advertises 2000000, the contract charges 10000000
>       min_buy_in: lobby advertises 200000000, the contract charges 1000000000
>       max_buy_in: lobby advertises 1000000000, the contract charges 5000000000
>       action_timeout_secs: lobby advertises 45, the contract charges 60
>       name "6-Max - 0.05/0.10" quotes a price...
> ```
>
> and green after healing. **Both lobby scenes' cause is gone**: the app in a plain browser no longer
> shows `⚠ 2 records differ`, `⚠ RECORD DIFFERS` or the strikethroughs.
>
> **This gate found [H-55](#h-55) while being written**, which is the more important result of the
> two: the script it was added to had been comparing one field in eight.

**Status: executed** against the local lobby and all four table canisters.

```
lobby get_tables()          id 1 "Heads Up - 0.01/0.02"  cfg 1_000_000/2_000_000   200_000_000/1_000_000_000  2 seats
                            id 2 "6-Max - 0.01/0.02"     cfg 1_000_000/2_000_000   200_000_000/1_000_000_000  6 seats
                            id 3 "9-Max - 0.01/0.02"     cfg 1_000_000/2_000_000   200_000_000/1_000_000_000  9 seats
table_1 get_table_view()        1_000_000/2_000_000    200_000_000/1_000_000_000   2 seats   <- agrees
table_2 get_table_view()        5_000_000/10_000_000   1_000_000_000/5_000_000_000 6 seats   <- name is 5x wrong
table_3 get_table_view()       10_000_000/20_000_000   2_000_000_000/1e10          9 seats   <- name is 10x wrong
```

Two separate defects wear one symptom, and the lobby scenes are red for both:

1. **The registered NAME quotes a price the contract does not charge** — 4 mismatching figures in the
   rows plus 2 in the preview heading. The client already quotes the name verbatim (it is the table's
   registered identity), strikes the contradicted figure through and labels it `STALE NAME`; the
   harness reads `textContent` and is right to, because a struck figure is still a figure on screen.
2. **The registered CONFIG disagrees with the contract** on `smallBlind`, `bigBlind`, `minBuyIn` and
   `maxBuyIn` for ids 2 and 3 → 2 `structuralProblems`, which fail the scene independently of (1).

**Every price the client computes is the contract's**, verified figure by figure this pass
([DESIGN-BAR BAR 30](DESIGN-BAR.md#6-the-bars-cleardeck-has-to-clear)): stakes, buy-in ranges, blinds, clock and ante
all agree with `get_table_view().config`. The client reads the table canister and treats the lobby
record as a fallback, which is why nothing else on the screen is wrong.

**What (1) needs — exists today, one call per table:**

```
icp canister call lobby update_table_name '(2 : nat64, "6-Max - 0.05/0.10")' -e local --identity <lobby admin>
icp canister call lobby update_table_name '(3 : nat64, "9-Max - 0.10/0.20")' -e local --identity <lobby admin>
```

**What (2) needs — a method that does not exist.** `lobby_canister` exposes `update_table_name` and
nothing that can rewrite a registered `config`; `init_microstakes_tables` only re-writes the same
hardcoded 0.01/0.02 record for all three tables. Either add

```candid
update_table_config : (nat64, TableConfig) -> (Result);   // admin only, mirrors update_table_name
```

or — better, and what the harness's own message recommends — make `init_microstakes_tables` take the
configs it registers (or read each one from the table canister) so the registry cannot drift by
construction. **The engine was left alone this pass, by instruction.**

**Who can make the calls, which is its own finding.** The local lobby's admin is the identity that
deployed it (`init()` sets `ADMIN = msg_caller`), which on this machine is the default identity, not
the project's `cd-local-deployer`. Executed proof:

```
lobby get_admin()                                    -> opt principal "nsp5q-…-5ae"
icp identity principal --identity cd-local-deployer   -> sg3sw-…-6qe
lobby update_table_name(2, …) --identity cd-local-deployer
                                                     -> (variant { Err = "Unauthorized: admin only" })
```

So `up_wire()` in `scripts/dev.sh` — which calls `set_admin(cd-local-deployer)` and then
`init_microstakes_tables(...)`, both behind `|| warn "… (already set?)"` — **cannot administer the
lobby on this replica and has been failing silently**. `set_admin` refuses a non-admin caller, so the
warning is not "already set", it is "not authorised". That is `scripts/dev.sh`'s owner's call to make;
it is recorded here because it is the reason the two calls above could not be issued this pass (the
only identity that holds admin is the machine's default identity, and signing a write with it was
refused by this session's permission policy).

Durable fix, once the method exists: put both `update_table_name` calls and the config registration
in `up_wire()` next to `init_microstakes_tables`, and in `scripts/deploy-mainnet.sh` next to its own
`init_microstakes_tables` call — otherwise every `make local-up` and every mainnet deploy re-registers
the stale strings.

<a id="l-05"></a>
### L-05 — low — T-05 determined: `btc_table_1` needs a canister call, not a frontend change — STATUS: OPEN

**Status: executed** (read the registry and the BTC table's own config; the write itself was not
issued — see [L-04](#l-04) on who holds admin).

The frontend is already complete for a BTC table: `Lobby.svelte` reads `currency` off the record and
the live view, formats sats (`unitOf`, `formatAmount`), renders the ₿ chip, tiers BTC stakes
separately (`stakeTier`), and shows the currency filter as soon as a second currency appears. It
renders whatever `lobby.get_tables()` returns, and that call returns three ICP tables and nothing
else. **So this is not fixable in the frontend: the row does not exist.**

The call that fixes it, exactly:

```
icp canister call lobby add_btc_headsup_table '(principal "46el7-ql777-77775-aaada-cai")' \
  -e local --identity <lobby admin>
```

`add_btc_headsup_table` (`src/lobby_canister/src/lib.rs:339`) is admin-only, takes the next free id,
and registers `100/200` sats with a `10_000–100_000` sat buy-in, 2 seats, `Currency::BTC` — which is
**exactly** what `btc_table_1`'s contract enforces (verified: `small_blind 100`, `big_blind 200`,
`min_buy_in 10_000`, `max_buy_in 100_000`, `max_players 2`, `currency BTC`). Its registered name,
`"Heads Up - 100/200"`, quotes those same blinds, so unlike ids 2 and 3 this row would arrive with
**no** [L-04](#l-04) drift and no stale name. No engine change is needed, and none was made.

Durable fix: one line in `up_wire()` in `scripts/dev.sh`, next to the `warn` that currently states
the defect, and the same call in `scripts/deploy-mainnet.sh`. Both are other owners' files.

**What it costs the layout, stated up front so it is not a surprise.** A second currency turns on the
currency filter row: 8 pills instead of 5. Simulated in the page at 390×844, the strip **wraps and
cuts nothing** (BAR 28 holds), but it costs 35 px and the phone then shows **1 of 4 cards** instead
of 2 — so on a phone T-05 and [L-03](#l-03) should land together.

<a id="l-06"></a>
### L-06 — medium — the occlusion gate's manifest trips `make hygiene` — STATUS: FIXED (wave 7)

**Status: executed.** `./scripts/dev.sh shots` (full run, 22 shots) writes
`artifacts/screens/fe72d46/manifest.json` at **5,575,176 bytes**. `./scripts/dev.sh hygiene` fails
on it:

```
==> no large or binary files added
    37 untracked path(s), 6633 KiB total
    ! untracked payload exceeds 4 MiB; check for a stray artifact directory
```

Where it comes from, per shot: `occlusion` **60,181 bytes** on the lobby alone, against
`tokenCensus` 26,321 and `chain` 11,883. The gate records every pixel-tested pair, and 22 shots of
that is the whole 5.58 MB.

This is not the gate being wrong — the pixel evidence is exactly what makes it a gate rather than a
claim — but a harness that leaves the repo failing its own hygiene check trains people to ignore
hygiene, which is the same failure mode the wave-4 lesson is about. Options for the owner: keep the
per-pair detail only for FAILING figures and a count for the rest; or write the pair-level detail to
a sibling file that hygiene's payload check does not count.

Not touched here: `tools/shots/lib/occlusion.mjs` and `capture.mjs` are another owner's files, and
deleting the artifacts would delete another agent's evidence.

---

## Found by the wave-5 coherence pass

Two sources. The first is an **independent auditor** given the running local stack, the public
repository and no access to any of these documents, told to treat every claim in them as marketing.
Its verdict is quoted in full in [WAVE-05.md](WAVE-05.md). Its findings are triaged below with the
same rule the wave-2 pass used: *anywhere the auditor was confused, misled, or could not verify
something, that is a defect in the product, not a misunderstanding to explain away.* Where we
believe the thing it could not verify is nevertheless true, the defect is that it is
**undiscoverable**, and that has a different fix (say it, and give a reader a way to check it) from
the defect of being false.

The second is this pass's own walk of the app: every view, both viewports, on the rendered page.

> **HISTORICAL SNAPSHOT — the status column below is NOT maintained.** It is the queue as it was written in this wave. The only true statuses are in [THE REGISTER](#the-register).

| # | sev | status | where | one line |
|---|---|---|---|---|
| [E-42](#e-42) | **critical** | executed by the auditor on a funded local table; call site confirmed by code read | `record_hand_to_history` → `poker_core::evaluate_hand` (`src/table_canister/src/lib.rs:871`) | **a funded table was bricked with ~420 ICP unreachable through every path a player has.** One player stopped heartbeating pre-flop; from then on `check_timeouts`, `player_action` and `leave_table` all trapped (`IMPOSSIBLE HAND: … got 0 community cards`), while `withdraw` and `cash_out` refused with *"Cannot withdraw while in a hand"*. The one unguarded settlement-path call to a trapping evaluator. [FINDING 15](SECURITY-FINDINGS.md) |
| [T-33](#t-33) | **critical** | executed by the auditor | `Dockerfile`, `scripts/verify-build.sh`, `README.md` §Verify the Code, `icp.yaml` `shrink` | **nobody can check what code is running.** The deployed module hash matches no commit in the repository; the build is not path-independent (a 274 KB `name` section survives `shrink`); the published Docker verification cannot compile because its `COPY` list omits `src/poker_core`, which `table_canister` depends on; and the hash-reading procedure the README tells a reader to run is controller-only |
| [E-43](#e-43) | **high** | code-read confirmed; exploited by the auditor in play | `count_players_can_act`, `count_active_players`, `is_betting_round_complete`, `check_timeouts` | a seated player who stops heartbeating is dropped from the betting round and **stays fully eligible for the pot**. Client-controlled, so it is an exploit: call the flop, stop heartbeating, get the turn and river free with full pot equity and immunity from any further bet |
| [E-44](#e-44) | **high** | **FIXED IN THIS PASS** (both orders re-called on the running replica) | `verify_shuffle : (text, text) -> (bool) query` | the one on-chain call a non-technical player would reach for to check they were not cheated **answers "false" for a genuine proof** when the two hex strings are passed in the order a reader would pick. No parameter names in the Candid, `bool` return, so *"you called it backwards"* and *"you were cheated"* are the same answer |
| [T-36](#t-36) | **high** | **FIXED IN THIS PASS** (measured red first, on the rendered page) | `+page.svelte`, `DepositModal.svelte`, `WithdrawModal.svelte`, `HowItWorks.svelte` | **HARD RULE 2 was live-broken across the whole desktop app.** The canonical sentence *"No rake is taken from any pot on any table"* was on screen on **4 of the 15 (surface, viewport) pairs** a player can reach, and on **1 of the 7 desktop ones**. Desktop lobby, desktop table, desktop table behind Deposit, desktop table behind Verify Fair: **4 of 5**. Portrait with FULL TERMS open, portrait behind Deposit: **4 of 5** |
| [H-40](#h-40) | **high** | **FIXED IN THIS PASS** | `tools/shots/run.mjs`, new `tools/shots/lib/felt-area.mjs` | wave 5 built a pixel-level notice gate and never wired it in: `protected-notices.mjs` was imported by **two** scenarios and by nothing else. Flipping one declaration, `.banner-strip { display: block }` → `display: none`: re-commits wave 4's exact crime with **every gate in the repo green**. Both halves now run centrally, for every scene at every viewport |
| [T-34](#t-34) | medium | **FIXED IN THIS PASS** (101 hands archived, then the table's copy destroyed twice over) | history canister `4xhad-gd777-77775-aaacq-cai`; `MAX_HAND_HISTORY_ENTRIES = 100`; `reset_table` | the "permanent hand history" canister is deployed, **authorised for no tables, holding zero records**, and no table is wired to it (`get_history_canister() = null` on `table_2`). Proofs live only in the table, capped at 100 hands, pruned, and wiped by one admin call |
| [E-77](#e-77) | medium | code-read | `notify_deposit` (archived-block branch) | a deposit made by plain transfer can only be credited while its block is still resident in the ledger canister. Once archived it can **never** be claimed, and `admin_restore_balance` was deliberately removed, so nothing can credit that user afterwards. The README documents this path |
| [E-46](#e-46) | medium | executed | `icrc1_balance_of` vs `admin_get_all_balances` + `admin_get_table_chips` + `get_pot` | no endpoint reconciles **funds held** against **liabilities recorded**. `table_2` holds 734,105,000,000 e8s against ~42,000,000,000 of recorded liabilities. Nothing is under-collateralised, but *"held by the canister and attributed to nobody"* is exactly what a lost deposit looks like and there is no view that tells the two apart |
| [T-32](#t-32) | medium | executed (new, this pass) | `PokerTable.svelte:313` `max_players ?? 9` | **every table entry first paints a NINE-seat ring.** At a 6-max table on a phone the felt is 45.3%→50.7% of the frame for **336 ms** and then jumps to 60.6%; on desktop 33.2% for **304 ms** and then 31.7%. Wave 5's headline 60.6% is the settled state and was never distinguished from the first paint |
| [H-39](#h-39) | medium | executed (new, this pass) | `scripts/dev.sh` `cmd_hygiene`, size rule | `make hygiene`'s payload check counts **modified tracked files** as untracked payload (1,056 KiB of them right now), so a large wave plus one screenshot run makes it red for a reason that has nothing to do with large or binary files. Two wave-5 agents reported *"repo hygiene clean"* and two critics found it red; both were right, at different times |
| [E-47](#e-47) | low | executed | `TableView` in `table_canister.did` | nothing distinguishes **caller-relative** fields (`can_check`, `call_amount`, `is_my_turn`, `min_bet`) from **global** ones (`action_on`, `current_bet`, `phase`), and no field says what the seat on action may legally do. The auditor read `can_check = true` while the seat on action owed 5,000,000 and got *"Cannot check, there's a bet to call"* eight times |
| [E-48](#e-48) | low | executed | `set_display_name` | reserved UI words are not rejected: `set_display_name(opt "You")` and `(opt "Dealer")` both return `Ok`. The auditor read a seat with `is_self = false` and `display_name = opt "You"` |
| [T-35](#t-35) | low | executed | local replica: no ckBTC ledger at `mxzaz-hqaaa-aaaar-qaada-cai` | `btc_table_1` is deployed locally but the ledger it needs is not, so **half the custody surface has no local test path**. Its mainnet twin is documented as holding real ckBTC |
| [H-42](#h-42) | **high** | executed (new, this pass) | `scripts/dev.sh` `cmd_test` step 4 | **`./scripts/dev.sh test`, the repo's primary gate, hung for 33 minutes** with zero CPU on both the test binary and its own PocketIC. The money-safety targets have NO time bound; the settlement targets one step later have two. Every leg is green when run directly (`invariants` 45/0 in 63 s single-threaded) |
| [H-41](#h-41) | medium | **FIXED IN THIS PASS** | `chain-agreement.mjs`, `token-census.mjs`, `token-allowlist.mjs` | both `deposit` shots were filed UNVERIFIED on every run: the `18` of a player-protection notice read as an unasserted money figure, and T-30's new `0.0004` wallet requirement asserted by nothing. The money figure is asserted now, not allowlisted |
| [H-43](#h-43) | medium | executed (new, this pass) | `capture.mjs` `writeManifest` / `writeIndex` | a partial screenshot run **overwrites the full run's `INDEX.md` and `manifest.json`** with only the scenes it ran, in both `<sha>/` and `latest/`. The PNGs survive; the record of what they prove does not, and nothing warns |
| [H-44](#h-44) | medium | measured (new, wave 7) | `money_safety::fuzz::next_op` | the fuzz GENERATOR cannot produce the E-36 / FINDING 17 sequence. `JoinTable` + `SitIn` exist as ops, but the sequence needs a LIVE hand, an EMPTY chair and both calls in order, and 220 hostile steps at three seeds produced it zero times. M11 OUTCOME fires on the deterministic reproducer and is silent on every fuzz seed with the defect deliberately restored -- so the class is gated by the scripted probe, not by search |
| [D-06](#d-06) | medium | item 1 **FIXED IN THIS PASS** in README/spec/`ShuffleProof.svelte`; still open in `Lobby.svelte` and `HowItWorks.svelte`. Item 2 is [T-33](#t-33) | `README.md` lines 95-125, `docs/SHUFFLE-SPEC.md` | two claims a stranger reads as stronger than they are: *"the commitment is published before the deal"* (true only inside a single message, `start_new_hand` commits and deals atomically, so no outsider can observe the commitment before cards exist), and *"You can verify that the deployed canisters match this source code"* (addressed to people who by construction cannot run the procedure) |
| [E-49](#e-49) | **high** | **FOUND AND FIXED INSIDE THIS PASS**, measured on the replica | `history_canister` de-duplication key | the first idempotency key was `(table_id, hand_number)`. `reset_table` restarts hand numbering at zero, so after a reset four genuinely new hands were **silently discarded as duplicates** while the table was told `Ok` four times: `get_total_hands` moved 4 -> 5. Now keyed on the seed hash too |
| [E-50](#e-50) | medium | **FIXED for the archive wiring, OPEN for the lobby wiring** | `scripts/dev.sh` `up_wire` | `icp canister call` **exits 0 when the method returns `variant { Err }`**, and every controller-only call in the script relied on the exit code. That, plus `$CONTROLLER` not actually being the controller, is why T-34 held for a whole wave with the script printing success |

### What the auditor confirmed, unprompted

Recorded because the wave's headline claims should be gradeable by a stranger, and here three of
them were:

- **The shuffle is verifiable by an outsider.** The auditor wrote its own verifier from
  `docs/SHUFFLE-SPEC.md` alone, without reading or running any of ours, and reproduced three hands
  from the running canister exactly: every hole card, every board card, a folded seat's cards the
  canister never published, and a hand where it recorded the commitment at the flop and then
  **predicted the turn and river**. *"The spec is precise enough that I had to guess nothing."*
- **No rake, and chips conserved to the e8** in every hand it settled.
- **`withdraw` is properly guarded against reentrancy**, the admin queries reject non-controllers,
  mucked cards stay hidden, and the dev faucet really is dead.

<a id="e-42"></a>
### E-42, critical, one unguarded evaluator call bricks a funded table — STATUS: FIXED (wave 5)

> **UPDATE 2026-08-05.** Reproduced end to end on the running local replica against the byte-exact
> deployed module (an isolated canister, `5tkpr-7d777-77775-aaaeq-cai`, so no other agent's fixture
> was touched): every door shut at once, exactly as reported.
>
> **The call site named below is the WRONG ONE.** It was picked by reading the source for the one
> unguarded `evaluate_hand`. The canister's own log names the real one, and it is on the
> **fold-out** path, not the showdown path:
>
> ```text
> poker_core::hand::evaluate_hand
> table_canister::plan_payouts          <-- rank_claims, not record_hand_to_history
> table_canister::settle_hand
> table_canister::end_hand_single_winner
> canister_update leave_table
> ```
>
> **The cause is not the evaluator.** The engine ended a hand as "everybody else folded" while two
> seats still held live claims and there was no board to rank them by, because
> `count_active_players` (which ends the hand) filtered on `status == Active` while `live_claims`
> (which decides who may win) filters on `has_folded`. That is [E-32](#e-32)/[E-06](#e-06), fixed
> in this wave by the "WHO IS IN THE HAND" change: 762 of 4,000 randomised endpoint sequences
> reached the impossible state before it, 0 of 4,000 after.
>
> **A guard alone would not have been a fix.** With the board-length guard and without the cause
> fix, the same state settles by refunding *every* funder including the players who folded — the
> hand is silently un-played, conservation is exact, and M1–M8 are all silent. Measured.
>
> **Also landed here:** all three settlement-path evaluator calls now use `try_evaluate_hand` and
> `evaluate_hand` is not imported into `table_canister` at all (greppable rule, asserted every
> run); `abandon_stuck_hand()` gives any caller a door that does not depend on the settlement path;
> `withdraw`/`cash_out` stop refusing once the hand is provably stuck; and `apply_payouts`'s false
> claim that trapping there is "recoverable" is corrected. Gated by **M9 FUND REACHABILITY**
> (`tests/money_safety/tests/fund_reachability.rs`), which goes 5-of-6 RED when the fix is
> reverted. Full write-up: [FINDING 15](SECURITY-FINDINGS.md).
>
> **Not fixed by this:** [T-33](#t-33). The module holding money is still not the source.

**Status: executed by the independent auditor on local `table_2`
(`4zfnl-5t777-77775-aaadq-cai`), deployed module `0x5298915c…`. The call site named below was
confirmed by code read in the current source and was WRONG; see the update above. Full write-up:
[FINDING 15](SECURITY-FINDINGS.md).**

What the auditor did was play a normal hand. One seated player's client stopped sending heartbeats
for thirty seconds while a pre-flop hand was live. From that moment:

| call | result |
|---|---|
| `check_timeouts` | IC0503 trap: `IMPOSSIBLE HAND: evaluate_hand needs a 3-, 4- or 5-card board (flop/turn/river), got 0 community cards` |
| `player_action` | same trap |
| `leave_table` | same trap |
| `withdraw` | `Err("Cannot withdraw while in a hand")` |
| `cash_out` | `Err("Cannot cash out while in a hand")` |

38,000,000,000 e8s of escrow, 3,985,000,000 of chips and a 15,000,000 pot, about **420 ICP** , 
were unreachable through **every path a player has**, simultaneously. It cleared only because the
auditor happened to try `sit_out`, which does not advance the game and is suggested by no error
message.

**The mechanism, confirmed in the source at HEAD.** `poker_core::evaluate_hand` traps by design on
an impossible input (E-09's fix; the block comment argues for the trap and offers
`try_evaluate_hand` for callers who want the rejection). Three call sites in `table_canister`
reach it from settlement. Two of them guard the board length:

- `src/table_canister/src/lib.rs:4098`: `if !(3..=5).contains(&state.community_cards.len()) { return Vec::new(); }`
- `src/table_canister/src/lib.rs:4505`: `.filter(|_| state.community_cards.len() >= 3)`

The third does not:

```rust
// src/table_canister/src/lib.rs:871, inside record_hand_to_history
final_hand_rank: if show_cards {
    p.hole_cards.as_ref().map(|cards| evaluate_hand(cards, &state.community_cards))
} else {
    None
},
```

`record_hand_to_history(state, &winners, true)` is called unconditionally by `determine_winners`
(`:4631`), and `show_cards = went_to_showdown && !p.has_folded`. So **any** path that reaches a
showdown with fewer than three community cards traps, and both street-advance routines make that
state reachable, because they set the next phase whether or not they managed to deal:

```rust
// advance_to_next_street, GamePhase::PreFlop
if state.deck_index + 3 < state.deck.len() {   // deal the flop
    …
}
state.phase = GamePhase::Flop;                 // ← unconditional
```

`run_out_board` has the same shape at every street. Once the phase reaches `Showdown` with a short
board, the trap is **permanent**: the trap rolls the message back, the state is unchanged, and the
next call takes the same path.

**Why this is the finding that matters more than its own severity.** The auditor replayed the exact
bricked state against the current source in its own host harness and it settled correctly. So the
software offered for inspection and the software holding money are different, and the one holding
money is worse, which is [T-33](#t-33), and which is why T-33 is filed as critical and not as
housekeeping.

**The fix, and why this pass did not apply it.** One line: `try_evaluate_hand(cards,
&state.community_cards).ok()` at `:871`, which makes the history record lossy instead of fatal. The
deeper fix is the auditor's own recommendation, *"make every settlement evaluator call refuse
rather than trap"*: plus not treating a missing heartbeat as an absence of obligation ([E-43](#e-43)).
Neither is applied here: this pass owns no engine file, the engine is byte-identical to `fe72d46`
by design, and HARD RULE 3 says a fund finding is written up prominently and **never quietly
patched**. It needs a failing test first (a `TableState` at `Showdown` with an empty board, settled
through `determine_winners`), and that test does not exist yet.

<a id="t-33"></a>
### T-33, critical, a stranger cannot tell what code is holding their money — STATUS: FIXED (wave 9)

**Status: executed by the independent auditor.** Four independent breaks in one story:

1. **The deployed module matches no commit.** The auditor rebuilt `table_2` from the working tree
   and from `fe72d46`, `3253b67`, `7bc69db` and `801aa79` and got `4ca176e3`, `3ac83432`,
   `427e9f60`, `b87cc31a`, `93c505ea` against the deployed `5298915c…`. The deployed module's
   **code section** is 1,758,178 bytes against 1,773,482 for every HEAD build, so it is different
   code, not different metadata.
2. **The build is not path-independent.** Identical source at two directories produced `3ac83432`
   and `78e90a11`, differing only inside the 274 KB `name` custom section that `shrink: true` fails
   to strip. This is [H-29](#h-29) again, from outside.
3. **The published Docker verification cannot start.** `Dockerfile:23-26` copies `icp.yaml`,
   `Cargo.toml`, `Cargo.lock`, `src/table_canister`, `src/lobby_canister` and
   `src/history_canister`: and **not `src/poker_core`**, which is a workspace member
   (`Cargo.toml:3`) and a path dependency of the table canister
   (`src/table_canister/Cargo.toml:19`). `icp build` dies with *"failed to load manifest for
   workspace member …/src/poker_core"*. Confirmed here by reading both files.
4. **The procedure the README gives a reader is controller-only.** `scripts/verify-build.sh` reads
   deployed hashes with `icp canister status <id> -e ic`. The README labels this "(controller-only)"
   in one block and then tells the reader to run exactly that in "Manual Verification Steps" step 1
   with no caveat. The script's own closing note, *"confirm the exact 'Module hash' field name …
   on first run"*: says it had never been run. The public read-state path for `module_hash` exists
   and is not used.

**Why critical rather than medium.** Everything else in this repository describes source. E-42
shows the deployed binary behaving worse than the source on the same state. A verifiable shuffle
inside an unverifiable binary buys a player very little, and anyone who audited this repository
audited software that is not deployed.

<a id="e-43"></a>
### E-43, high, a missing heartbeat is treated as an absence of obligation — STATUS: FIXED (wave 6)

**Status: code-read confirmed here; exploited in play by the auditor.**

```rust
// src/table_canister/src/lib.rs:2880-2892
fn count_active_players(state: &TableState) -> usize {
    … .filter(|p| … !p.has_folded && p.status == PlayerStatus::Active) …
}
fn count_players_can_act(state: &TableState) -> usize {
    … .filter(|p| … !p.has_folded && !p.is_all_in && p.status == PlayerStatus::Active) …
}
```

`check_timeouts` flips `Active → Disconnected` after 30 s without a heartbeat
(`:4667-4669`) and does **not** fold the seat. So the seat leaves every "who still has to act"
count while remaining un-folded and therefore eligible for the pot. Live evidence from the auditor:
`table_2` hand 1 recorded exactly one action for the whole hand (seat 1 `Fold`), and the canister
dealt the complete board and evaluated seat 2's hand at showdown even though seat 2 was 5,000,000
short of the 10,000,000 current bet and had never acted.

It is client-controlled, which is what makes it an exploit rather than a bug. This is the same root
as [E-06](#e-06) (one lull runs the whole board out) seen from the attacker's side.

<a id="e-44"></a>
### E-44, high, the fairness endpoint answers "false" for a valid proof — STATUS: FIXED-NO-GATE (wave 5)

**Status: FIXED. Verified on the running local replica, both ways round, against a hand the
canister itself dealt.** The fix is `check_shuffle_commitment`, which takes a **record** — Candid
keys record fields by hashed name, so there is no argument order left to get wrong — and answers a
**variant that names the fault**. `verify_shuffle` is kept, deprecated, and made order-insensitive
so an existing caller cannot be handed a false accusation by it.

`table_1`, hand 1, seed hash `41779ced…c9a8`, revealed seed `2b8a1a4a…235a` (independently confirmed
with `shasum`, not with the canister):

```
$ icp canister call table_1 verify_shuffle '("<seed>", "<hash>")' --query   # natural order
(true)                                    # before this pass: (false)
$ icp canister call table_1 verify_shuffle '("<hash>", "<seed>")' --query   # documented order
(true)

$ icp canister call table_1 check_shuffle_commitment \
    '(record { seed_hash = "<hash>"; revealed_seed = "<seed>" })' --query
(variant { Match = record { computed_hash = "41779ced…"; …
   this_does_not_prove = "Nothing about fairness. This is the table checking its own homework…" } })

$ … '(record { seed_hash = "<seed>"; revealed_seed = "<hash>" })'   # values transposed
(variant { FieldsSwapped = record { … } })                          # NOT a failed proof, and says so

$ … seed_hash from hand 1, revealed_seed from hand 2
(variant { NoMatch = record { computed_from_revealed_seed = "b97effde…";
   computed_from_seed_hash = "1cb71ac8…"; meaning = "Neither value hashes to the other…" } })

$ … revealed_seed = "2b8a1a4ab2910e494"           # truncated paste
(variant { Malformed = record { field = "revealed_seed"; character_length = 17;
   reason = "an odd number of hex characters…" } })

$ … seed_hash = "4fbx2-kt777-77775-aaabq-cai"     # a principal in the hash field
(variant { Malformed = record { field = "seed_hash";
   reason = "not hexadecimal. Expected only the characters 0-9 and a-f…" } })
```

The pre-fix arithmetic, for the record: the old body compared `SHA256(bytes(arg2))` with `arg1`, so
the natural reading gave `SHA256(hash) = 1cb71ac8…` against `seed = 2b8a1a4a…` — **false, for a
genuine proof**.

Every arm carries `this_proves` and `this_does_not_prove` in plain text, because the deeper half of
the finding is that the endpoint is close to meaningless whatever it returns: the honest
verification is the one that runs on the player's machine. The fairness panel's own use of it is
labelled the same way and now shows **both** orders side by side, so the trap is visible rather
than merely absent.

The original evidence, unchanged:

```
src/table_canister/table_canister.did:322   verify_shuffle : (text, text) -> (bool) query;
src/table_canister/src/lib.rs:5326          fn verify_shuffle(seed_hash: String, revealed_seed: String) -> bool
```

The Candid exposes **no parameter names**. A reader with a seed and a hash from a hand the canister
itself dealt has a 50% chance of calling it in the order that returns `false`, and the return type
is `bool` rather than a `Result`, so a wrong argument order and a rigged table are indistinguishable
answers. The auditor: *"That is worse than having no endpoint."*

Cheap fixes, in increasing order of goodness: name the parameters in the Candid; return
`Result<(), String>` naming which argument failed to parse; or accept the pair in either order and
say which reading matched.

<a id="t-36"></a>
### T-36, high, the no-rake sentence was on screen on one surface of nine — STATUS: FIXED (wave 5)

**Status: FIXED. Measured red first, on the rendered page, with the repo's own gate.**

`tools/shots/lib/protected-notices.mjs` lists five protected phrases and
`routes/+page.svelte:747-749` records why the fifth exists: stating the no-rake property only as
*"No middleman, no house"* was judged **not** to discharge it. Wave 5 then put the canonical
sentence in exactly one place, `.banner-strip`, which is `display: none` at every viewport except
portrait-on-the-table-view.

Measured with `probeProtectedNotices` on the dist built from the tree as the four wave-5 agents
left it, both viewports, real canisters:

| surface | 1440×900 | 390×844 |
|---|---|---|
| lobby, signed out | **4/5** | **4/5** |
| lobby + How it works | **4/5** | **4/5** |
| lobby, signed in | **4/5** | **4/5** |
| table, as it lands | **4/5** | 5/5 |
| table + FULL TERMS open | n/a (no strip) | **4/5** |
| table + Deposit modal | **4/5** | **4/5** |
| table + Hand History | 5/5 | 5/5 |
| table + Verify Fair | **4/5** | 5/5 |

The missing phrase was the same one every time. Two of these are worse than a bare omission:

- **The FULL TERMS sheet was not a superset of the strip it replaces.** A player who taps the strip
  to read the terms had the no-rake sentence taken off the screen by the act of asking for it:
  `.banner-content` covered `.banner-strip` and did not restate the property.
- **The money modals restated the property in the weaker wording** ("No middleman, no house, 0%
  rake"), so the canonical sentence stayed outside the dialog, behind the very scrim the in-dialog
  notice was built to escape.

**Fix.** Purely additive, four files, no existing notice line touched (`git diff fe72d46 --
README.md src/cleardeck_frontend/src | grep '^-' | grep -Ei 'unaudited|18\+|jurisdiction|rake'` is
empty):

- `+page.svelte`: `.banner-info` and `.disclaimer-info` now state the sentence, so it is on screen
  on every desktop view and in the FULL TERMS sheet.
- `DepositModal.svelte`, `WithdrawModal.svelte`, `HowItWorks.svelte`: the sentence added on its own
  line beneath the existing wording.

**Re-measured, same probe, same canisters: 5/5 on all 15 (surface, viewport) pairs.** It is now
also asserted by the harness on every scene ([H-40](#h-40)) rather than by a script in a scratchpad.

**What it cost, honestly.** One extra line in `.banner-info` at 1440×900 pushes the stage down
18 px, so the desktop 6-max felt goes 961.1×457.7 → **929.3×442.5** (33.9% → 31.7% of the window).
Portrait is unchanged at 332.8×599.5 = 60.6%, because the strip carries the sentence there and the
strip was already three lines. HARD RULE 2 makes that trade non-negotiable, and the 18 px can be
bought back later out of non-protected copy in the same paragraph; it must never be bought back out
of a notice.

<a id="h-40"></a>
### H-40, high, the notice gate existed and was wired to nothing — STATUS: FIXED (wave 5)

**Status: FIXED.** `tools/shots/lib/protected-notices.mjs` is good code, written this wave, and
before this pass it was imported by `scenarios/handhistory.mjs` and `scenarios/handreplay.mjs` and
by **nothing else**: not `run.mjs`, not one table scene, not the lobby, not deposit. There was also
no felt-area assertion anywhere in `tools/shots`: wave 5's headline number, 60.6%, existed only in
prose and in a script in a scratchpad directory.

The cost of that was demonstrated by the pass's critic with a build rather than an argument: flip
one declaration, `.banner-strip { display: block }` → `display: none`, and the phone's table view
shows **zero of five** protected phrases, the felt rises to 61.6%, the occlusion gate reports 0
occluded on all six mobile scenes and `./scripts/dev.sh hygiene` prints *"repo hygiene clean"*.
That is wave 4's crime re-committed in one line with every gate in the repo green, the precise
structural failure this wave was chartered to end. Wave 4 was green because hygiene greps the
**source**; wave 5 was green because the probe that reads the **screen** lived in a scratchpad.

**Fix.** Both halves of HARD RULE 2 now run in `run.mjs` in the same central block as the token
census and the pixel gate, with no per-scene opt-in and no opt-out:

- `foldProtectedNotices(await probeProtectedNotices(page), …)`: every scene, every viewport. Below
  5 of 5 and the scene does not get its canonical filename.
- new `tools/shots/lib/felt-area.mjs`: measures `.felt` (the layout box of the visible surface, not
  `.poker-table`, which reads 68% on a desktop where the surface is 31.7%), asserts a floor per
  viewport (mobile 45%, desktop 28%) and **records** the exact geometry, aspect, pod count and ring
  class.

The pairing is the point, because the two failure modes are opposite and gating one invites the
other: hiding a notice makes the felt bigger, and shrinking the felt keeps the notices on screen.
`INDEX.md` gains a **NOTICES** column (bold below 5/5) and a **felt** column beside them.

Proved in both directions on the shipping tree, the mutation table is in
[WAVE-05.md](WAVE-05.md).

<a id="t-34"></a>
### T-34, medium, the fairness record has a shelf life of about 100 hands — STATUS: FIXED (wave 5)

**Status: executed here, read-only, on the local replica.**

```
icp canister call history get_total_hands       -> (0 : nat64)
icp canister call history get_authorized_tables -> (vec {})
icp canister call table_2 get_history_canister  -> (null)
```

The canister described as "permanent hand history (provably-fair shuffle records)" is deployed,
authorised for no tables, holds zero records, and no table points at it. Proofs therefore survive
only inside the table canister, where `MAX_HAND_HISTORY_ENTRIES = 100` with `drain(0..excess)`
prunes them and the controller's `reset_table` wipes them.

The auditor's framing is the right one: *"A fairness guarantee you cannot re-check tomorrow is not
a fairness guarantee."* Under an hour of heads-up play is enough to lose the record of the hand you
want to check.

**Status: FIXED, and the root cause was not the one the symptom suggested.**

The wiring code existed and had existed for waves. What was wrong was three separate things, each
invisible on its own:

1. **`up_wire` only ever made one of the two calls.** `set_history_canister` tells the table where to
   send; `authorize_table` tells the archive to accept. Only the first was in the script, so even a
   working table would have been refused.
2. **It made that call as an identity that is not a controller.** `$CONTROLLER` is
   `cd-local-deployer`; the backend on this machine was deployed by the default identity,
   `cyclepay-hotwallet`. `icp canister call` **exits 0 when the method returns `variant { Err }`** —
   a Candid `Result` is a value, not a transport failure — and the old line was
   `icp_local canister call … >/dev/null`, so `Err("Unauthorized: controller access required")` was
   discarded and the script printed `table_1 -> history <id>` and returned success.
3. **The table decoded the archive's reply with the wrong Candid shape.**
   `Response::candid::<R>()` is `decode_one`, and the code asked it for
   `(Result<u64, String>,)` — a one-field *record* wrapping the variant, which `record_hand` never
   sends. So **every** write, including ones the archive accepted and stored, came back as a decode
   failure. Measured on the replica mid-fix: 3 hands played, `get_table_hand_count` on the archive
   said 3, the table said `recorded_ok = 0, failed = 3`. Under the old code that was one
   `ic_cdk::println!` per hand and nothing else.

Fixes, all four measured on the running replica:

- `scripts/dev.sh` resolves the controller from `icp canister status` instead of asserting it,
  routes every controller-only call through `call_or_die` (which reads the reply and treats an
  `Err` as fatal), makes **both** wiring calls, and then **reads the wiring back off both
  canisters** as a postcondition. That postcondition is what found fault 2.
- `dispatch_hand_to_history` counts every outcome, keeps the last error verbatim, and **keeps the
  record** — bounded at 64 — instead of dropping it. `get_history_status` publishes all of it to
  anybody, unauthenticated. A table with no archive configured now buffers rather than returning
  early, so wiring the archive later recovers the hands.
- `flush_unrecorded_hands` re-sends the backlog and is callable by **any non-anonymous principal**,
  because the party with the strongest interest in the proof surviving is the player.
- `record_hand` on the archive is idempotent on `(table_id, hand_number, seed_hash)`, so retries
  cannot double-count `player_stats`.

Measured, end to end (`$SCRATCH/cap-and-destroy.mjs`, 101 real hands on `table_1`):

```
played 101 hands in 93s
  recorded_ok_since_start = 101   failed_since_start = 0   unrecorded_backlog = 0

=== forcing the 100-hand cap (periodic_cleanup) ===
local_history_len before cleanup: 101   after cleanup: 100
table  get_hand_history(1)          -> (null)          # hand 1 is GONE from the table
archive get_table_hand_count        -> (101 : nat64)
archive check_recorded_hand(10)     -> commitment_matches = true, seed intact

=== the admin key: reset_table ===
table  local_history_len            -> 0               # every local proof erased, one call
table  get_hand_history(1)          -> (null)
archive get_table_hand_count        -> (101 : nat64)   # untouched
archive still verifies hand 1: commitment_matches = true, seed present
```

And the archived hand 1 verified **without the canister**, which is the only check that counts:

```
$ echo -n 631a9dbb…9bb2 | xxd -r -p | shasum -a 256
284f60dda8751173b833b479c6391e61980f670c13e915bbef6abe35aa1172bd   # == the archived seed_hash
```

The recovery path was reproduced deliberately (`$SCRATCH/backlog-recovery.mjs`): archive switched
off by the controller, 3 hands played into the void (`backlog=3, failed=3`, `last_error` naming the
missing archive), wiring repaired, then **`cd-alice`, who is not a controller**, called
`flush_unrecorded_hands` and got `(variant { Ok = 3 })` — archive count 0 → 3, backlog 0. An
anonymous caller is refused.

**What is still true and is now SAID, in the UI and the README rather than only here:** the table
keeps 100 hands and one controller call erases them; the archive has no delete but a controller of
the archive can reinstall or delete the canister; and `authorize_table` can admit a writer that is
not a real table. `get_fairness_retention` (table), `get_retention_policy` (archive) and
`get_history_status` (table) state all of it from the canisters themselves, so the sentences on
screen are checkable rather than believable.

<a id="e-49"></a>
### E-49, high, an idempotency key that silently discarded four real hands — STATUS: FIXED (wave 5)

Recorded because it was **mine**, it was introduced and removed inside one wave, and the shape of it
is the interesting part.

The first version of the archive's de-duplication keyed on `(table_id, hand_number)`. That is wrong,
and wrong in the direction that loses evidence: `reset_table` restarts a table's hand numbering at
zero, so after any reset the next N hands collide with N older ones. Measured before the key was
widened — 4 hands played on `table_2`, the table received `Ok` **four times** because the archive
returns the existing id for a duplicate, and `get_total_hands` went **4 → 5**. One record stored,
three shuffle proofs silently gone, with a green status on both sides.

The key is now `(table_id, hand_number, seed_hash)`. The seed is 32 bytes of `raw_rand` per hand, so
two genuinely different hands never collide and a genuine retry always does. Re-measured: 4 hands,
`5 → 9`, `recorded_ok` +4, backlog 0.

The lesson generalises past this bug: **an idempotency key that is also a success signal turns
silent data loss into a green light.** Nothing in either canister would have reported this; it was
visible only because `get_total_hands` was being read on both sides of the same run.

<a id="e-50"></a>
### E-50, medium, `icp canister call` exits 0 on `variant { Err }`, and the deploy script relies on the exit code — STATUS: OPEN

**Status: FIXED for the archive wiring, OPEN for the lobby wiring, in the same function.**

A Candid `Result` is a *value*. `icp canister call c m '(…)' >/dev/null || die "…"` cannot see
`Err`, so every controller-only call in `scripts/dev.sh` that used that pattern reported success
whatever the canister said. That is fault 2 of [T-34](#t-34), and it is not confined to the history
wiring: `up_wire` still calls `lobby set_admin` and `lobby init_microstakes_tables` as
`--identity "$CONTROLLER"` with `|| warn "… (already set?)"`, which reads a refusal as an
idempotent no-op. The lobby is another agent's file this wave, so the pattern was left in place and
commented rather than changed silently.

`call_or_die` in `scripts/dev.sh` is the fix shape: run the call, read the reply, treat
`variant { Err` in the output as fatal, and print what the canister actually said. Anywhere in this
repo that pipes a controller-only `canister call` to `/dev/null` should be routed through it.

<a id="e-77"></a>
### E-77, medium, a deposit whose block has been archived can never be claimed — STATUS: OPEN

> **Renumbered in the wave-11 register pass.** This defect was filed as a second `E-45` by the
> wave-5 coherence pass, colliding with the currency-guard defect ([E-45](#e-45)) filed in wave 7.
> Ids are permanent and are never reused; this one is now **E-77**. Nothing outside this file
> linked to it.

**Status: code-read (the auditor's).** `notify_deposit` decodes `archived_blocks` as
`candid::Reserved` and discards it, and ends at *"Transaction not found at this block index (may be
archived)"*. Its comment says *"blocks at 33M+ should not be archived yet"*, which is a bet on the
ledger, not a guarantee: the ICP ledger archives continuously. There is no fallback, because
`admin_restore_balance` was deliberately removed ("unnecessary attack surface").

A user who follows the README's documented deposit path and then waits an hour before notifying
loses the deposit permanently, with no operator remedy. The mitigation is accidental: the shipped
UI uses `deposit()` and `claim_external_deposit()`, so only README followers and external
integrations are exposed. That makes the README the defect surface as much as the code.

<a id="e-46"></a>
### E-46, medium, no view reconciles funds held against liabilities recorded — STATUS: FIXED (wave 10)

**Status: executed by the auditor.** `table_2` holds 734,105,000,000 e8s against roughly
42,000,000,000 of recorded liabilities; `table_3` holds 2,731,830,000,000 against about
1,169,980,000,000. No table is under-collateralised, which is the good news and the whole of it.
A large surplus is attributed to no principal and there is no endpoint that reconciles the two
sides.

Given [E-77](#e-77) and the removal of `admin_restore_balance`, *"held by the canister but
attributed to nobody"* is exactly what a lost user deposit looks like, and nothing distinguishes it
from a shared test instance's residue. The auditor could not attribute the surplus, which is itself
the finding.

<a id="t-32"></a>
### T-32, medium, every table entry first paints a nine-seat ring — STATUS: OPEN

**Status: executed, new in this pass.** `PokerTable.svelte:313`:

```js
const maxPlayers = $derived(Number(tableState?.config?.max_players ?? 9));
```

Until the canister answers, `maxPlayers` is 9, so `seatCount` is 9, so the wrapper gets
`ring-crowded` and the felt resolves from the nine-seat formula. Measured with a
`requestAnimationFrame` sampler over the whole entry, on the real canisters, at a **6-max** table
(`table_2`):

| viewport | first paint | held for | settles to |
|---|---|---|---|
| 390×844 | 287.5×518.1 = **45.3%**, 9 pods, `ring-crowded` (rising to 50.7% as `--cd-avail` resolves) | **336 ms** | 332.8×599.5 = **60.6%**, 6 pods |
| 1440×900 | 950.3×452.5 = **33.2%**, 9 pods, `ring-crowded` | **304 ms** | 929.3×442.5 = **31.7%**, 6 pods |

The `?? 9` fallback is byte-identical at `fe72d46`, so this is pre-existing, not caused by wave 5.
What wave 5 changed is the size of the consequence: portrait 6-max and 9-max now resolve from
different formulas (`min(86cqw, 55cqh)` against `min(78cqw, 52cqh)`), so the correction is a
visible **9.4% linear jump** of the whole table rather than a nudge.

Two consequences worth separating:

1. **A player sees the table resize under them on every entry.** On a phone the ring is drawn for
   nine, then redrawn for six, a third of a second later.
2. **Any felt figure is timing-dependent unless it says which state it measured.** This pass's own
   first walk recorded 50.7% for the 6-max portrait table because it measured before the
   correction; the same probe on the same build reads 60.6% after it. Wave 5's headline is the
   settled state and never said so. The new felt gate records the pod count and the ring class
   beside every number for exactly this reason.

The fix is to render nothing ring-shaped until `max_players` is known, or to carry `max_players`
from the lobby row the player clicked (the lobby already has it), rather than guessing 9.

<a id="h-39"></a>
### H-39, medium, `make hygiene`'s size rule counts modified tracked files — STATUS: FIXED (wave 7)

**Status: executed, new in this pass.** `scripts/dev.sh:709-728`:

```sh
n="$(git status --porcelain --untracked-files=all | wc -l …)"
bytes="$(git status --porcelain --untracked-files=all | sed 's/^...//' | … )"
if [ "$bytes" -gt $((4 * 1024 * 1024)) ]; then warn "untracked payload exceeds 4 MiB"; …
```

`git status --porcelain` lists ` M path` as well as `?? path`, so every **modified tracked file**
is counted as untracked payload. Measured on this tree:

| what | KiB |
|---|---|
| what the rule counts | 2,199 |
| genuinely untracked (`??` only) | 1,143 |
| modified tracked files, wrongly counted | 1,056 |

Both conditions were then reproduced on the SAME tree, half an hour apart, by running the screenshot
suite in between:

```
before the definitive shots run:  42 untracked path(s),  1667 KiB total   ✓ repo hygiene clean
after  the definitive shots run:  44 untracked path(s), 13470 KiB total   ! repo hygiene FAILED
```

After the run, `artifacts/screens/fe72d46/manifest.json` is **5,972 KiB** on its own
([L-06](#l-06)), so genuinely-untracked payload is 6,209 KiB and the check fails **even with the
miscount fixed**. Both causes need addressing. **`make hygiene` and `./scripts/dev.sh shots` are
effectively mutually exclusive and whichever you ran last decides the verdict**: which is why two
wave-5 build reports say "repo hygiene clean" and two critics found `! repo hygiene FAILED`: both
were right, minutes apart.

Every substantive check in `cmd_hygiene`: no binary/media files, all four README notices, all four
frontend notices, the no-rake property, and "no notice line removed or altered since `ceacc37`" , 
is green on this tree and was green in every run either side of this pass. The failing rule is the
size heuristic alone.

Two-line fix, in `scripts/dev.sh` (not owned by this pass): filter to `grep '^??'` before summing,
and exclude `artifacts/screens/` from the payload count, which is where the rule's intent already
points ("check for a stray artifact directory").

<a id="e-47"></a>
### E-47, low, nothing says which `TableView` fields are caller-relative — STATUS: OPEN

**Status: executed by the auditor.** `can_check`, `call_amount`, `is_my_turn` and `min_bet` are
relative to the caller; `action_on`, `current_bet` and `phase` are global. Neither the type nor its
comments say so, and no field describes what the seat on action may legally do. Seated as a player
**not** on action, the auditor read `can_check = true` and `call_amount = 0` while the seat on
action owed 5,000,000, and acting on that produced *"Cannot check, there's a bet to call"* eight
times in a row. Any third-party client will make the same mistake and offer illegal actions.

<a id="e-48"></a>
### E-48, low, `set_display_name` accepts "You" and "Dealer" — STATUS: OPEN

**Status: executed by the auditor.** HTML and blank names are correctly rejected; reserved UI words
are not. `set_display_name(opt "You")` and `(opt "Dealer")` both return `Ok`, and on `table_1` the
auditor read a seat with `is_self = false` and `display_name = opt "You"`. Cheap seat-identity
spoofing in a money game.

<a id="t-35"></a>
### T-35, low, half the custody surface has no local test path — STATUS: OPEN

**Status: executed by the auditor.** `icp canister status mxzaz-hqaaa-aaaar-qaada-cai -e local`
returns *"Canister … was not found"*: the ckBTC ledger `btc_table_1` needs is not deployed on the
local replica. So no local deposit, claim, withdrawal or minter path can be exercised for BTC at
all, by anyone, including us. Its mainnet twin is documented as holding real ckBTC.

This is the same wall the wave-5 money-copy pass hit from the other side: the corrected 11-sat
minimum ([T-26](#t-26)) is right in the canister and in the source and **has never been seen on a
rendered page**, because there is no BTC row in the local lobby and Withdraw is disabled at zero
balance. Deploying a local ckBTC ledger is the one change that makes both verifiable.

<a id="t-38"></a>
### T-38, high, the "Deployed Canister Hashes" the app has been showing are three upgrades stale, and nothing in the repository compared them to anything — STATUS: FIXED (wave 9)

**Found while building the mainnet bundle, by reading the public dashboard index** (allowed
plain-HTTPS read; no mainnet canister was called).

`routes/+page.svelte` carried, hard-coded, under the heading **"Deployed Canister Hashes"**:

```
lobby   0xff6c893de860c5bd8dae85d67344ee94619fb6faad6d68b3265c9a6fe5a2cef8
tables  0x1b84e2fa1c35fd50001cb059ba644784fe5a6b36a093a2ac3e56c39bc3bbdf28
history 0xc9b1b78a6490cd2034b967dc9de11bb6377170e0e5ef96144b546da3a93dd8f9
```

`https://ic-api.internetcomputer.org/api/v3/canisters/<id>` reports, for the six live canisters:

```
kpfcd (lobby)    fee23e8e2e8a24ab0ee622039916f91b18ac2a78fbb40011ae533701d83443d1
kggj7 (history)  5736c9ac0d33959a11bcecf5c87881e579611d0cf4f2baa050e66a56f72657cc
kieex/lfkaz/lclgn/qrhly (tables)
                 5d57a1e9f17aedafb3ff33805b9a83266d8b6f44d7b02df8a2bf7f6078687fde
```

Not one of the three matched. **Nothing noticed, because nothing compared them**: the heading is a
claim about the state of the world made by a static file, and no build step, no test and no gate
read it. A verification surface whose numbers are never checked is worse than none — it is the
page a careful person reads *instead of* verifying.

The panel also printed `icp canister status <ID> -e ic` as the command to check with.
`canister_status` is a **controller-only** management call on mainnet, so the instruction was
unrunnable by every stranger it was printed for. [T-33](#t-33) made this exact point about
`scripts/verify-build.sh` two waves ago; the panel kept doing it.

**Fixed.** `$lib/deployed-build.js` holds the expected hashes as an explicitly labelled CLAIM,
with the date, who declared it, and the sentence *"this page was built on a machine that could not
read the deployed hashes for itself"*. The panel puts a **live reading beside it**, fetched on
demand in the reader's own browser from the public index (a party that is not us), with three
outcomes rendered: `match`, `MISMATCH` — *"do not deposit until that is explained"* — and
`unknown`, which is not the same as match. The printed command is a `curl` one-liner anybody can
run, plus `./scripts/verify-build.sh --mainnet` to reproduce the hashes from source.

**STILL OPEN FOR THE OPERATOR, AND IT IS NOT A UI QUESTION.** The hashes this wave was told are
deployed —
`lobby 0x7ee36baa…`, `history 0x59d4b80c…`, `tables 0x511c9d0e…` — are **not** the ones the public
index reports. They are what `EXPECTED_MODULE_HASHES` now contains, so the panel's own live check
will read MISMATCH on all six until that is resolved. Three explanations fit and this wave could
not distinguish them without calling mainnet: the index is lagging; the upgrade did not land; or
the two sets hash different bytes (raw `.wasm` versus the gzipped module actually installed).
`./scripts/verify-build.sh --mainnet` settles it.

<a id="t-39"></a>
### T-39, medium, local sign-in navigates to `http://undefined.localhost:4943` — STATUS: FIXED (wave 9)

Found by reading the emitted chunk of the mainnet build. `auth.js` built the local Internet
Identity origin as

```js
`http://${import.meta.env.CANISTER_ID_INTERNET_IDENTITY}.localhost:4943`
```

and **`import.meta.env.CANISTER_ID_*` does not exist in this build**. Vite exposes only
`VITE_`-prefixed variables on `import.meta.env`; `vite-plugin-environment` puts the
`CANISTER_`-prefixed ones on `process.env`. `canisters.js` reads all three spellings for precisely
this reason, three files away. The compiled literal is, verbatim:

```
"http://undefined.localhost:4943"
```

Two independent faults in one line: that, and the hard-coded `4943` when this project's managed
replica is pinned to **8077** ([T-03](#t-03)).

It survived because the screenshot harness authenticates with agent identities and never presses
Sign In, and because the MAINNET branch is unaffected (it takes `II_URL`). **Fixed**: the id is
resolved from all three spellings, the port comes from `LOCAL_GATEWAY_PORT`, and an unresolvable
id now **rejects with an actionable message** instead of opening a window on a host that cannot
exist. Note that nothing in `dev.sh` or the harness exports the local II id at all, so the local
sign-in path stays unusable until something does — the difference is that it now says so.

<a id="t-40"></a>
### T-40, high, 0 of 5 protected notices legible with the Verify Code dialog open, at both viewports — STATUS: FIXED (wave 9)

**Measured, not read**, by rendering the mainnet bundle and hit-testing each phrase on its own
pixels with `elementFromPoint` (`tools/shots/verify-mainnet-bundle.mjs`). Desktop 1440x900 and
mobile 390x844, identical result:

```
.strip-text          display:none              (portrait table strip; not this view)
.banner-warning      y -325  800x56            above the fold: opening this dialog from the
                                               footer link auto-scrolls the page
.disclaimer-warning  y  674  800x59            IN the viewport, under .modal-backdrop
                                               (rgba(0,0,0,0.8) + 4px blur, z-index 1000)
-------------------------------------------------------------------------------------
0 of 5 phrases on their own pixels
```

Every other dialog in this application was fixed for exactly this — DepositModal, WithdrawModal
and HowItWorks under [T-31](#t-31), HandHistory under [H-36](#h-36) — and **the one dialog that
was missed is the one whose entire subject is whether this deployment can be trusted.** The
scroll is what made it worse than the others: the top banner is not merely dimmed, it is off
screen, because the control that opens the dialog is in the footer.

**Fixed** by the established remedy: `.verify-modal` becomes a flex column with the body in a
scroller and the four notices restated verbatim in a `flex-shrink: 0` block outside it, so
arriving at the dialog is enough to have them on screen. Additional copy only; nothing anywhere
else weakened. Re-measured: 5/5 at both viewports, with the dialog open, and 5/5 at both viewports
on the lobby **underneath a real error toast**.

<a id="d-06"></a>
### D-06, medium, two claims a stranger reads as stronger than they are — STATUS: OPEN

**Status: executed by the auditor**, which is the point: it had no access to these documents and
still arrived at both caveats unprompted.

1. **"The commitment is published before the deal."** True only inside a single message.
   `start_new_hand` commits and deals atomically, so **no outsider can observe the commitment
   before cards exist**. What is genuinely provable, and what the auditor did prove, is that the
   whole 52-card order was fixed before the board was shown. The hand replayer's wave-5 rewrite
   already says the honest version ("the deck was already fixed at the moment you looked"); the
   README and the shuffle spec should match it.

   **Status: FIXED in `README.md`, `docs/SHUFFLE-SPEC.md` and `ShuffleProof.svelte`.** The spec's
   "what is proven" table now leads with *"the whole 52-card order was fixed before the board was
   shown"* and files the ordering claim under **NOT proven** with the reason; the README says the
   same and marks steps 3 and 5 of its own diagram as one message; the fairness panel's rung 1 is
   now *"The whole deck was fixed before the board came out"* instead of *"Before the deal, the
   table locked in a hash"*. Measured on the rendered page at 1440x900 and 390x844.

   **Still open elsewhere, and not this pass's files to edit:** `Lobby.svelte:1080` (*"The deck is
   committed before the deal and revealed after it"*), `Lobby.svelte:1112` (*"The shuffle-seed hash
   is published before a card moves"*) and `HowItWorks.svelte:125` (*"before any cards are dealt"*)
   all still assert the ordering. Same one-line change, three files.
2. **"You can verify that the deployed canisters match this source code."** Addressed to readers
   who by construction cannot run the procedure ([T-33](#t-33)).

One more, on the credit side, quoted because it is the most useful sentence in this section: *"The
README's own warning that funds are not safe is, as far as I can tell, the most accurate sentence in
the documentation."*

### Correction to [L-04](#l-04): the admin identity is on this machine

L-04 is filed as *"needs 2 admin calls **and** 1 lobby method that does not exist"* and tells the
next agent to build API surface. Verified read-only in this pass, with no state change:

```
icp canister call lobby get_admin        -> (opt principal "nsp5q-aelxk-…-5ae")
icp canister call lobby is_caller_admin --identity cyclepay-hotwallet -> (true)
icp canister call lobby is_caller_admin --identity cd-local-deployer  -> (false)
```

`cyclepay-hotwallet` is a local identity and it **is** the lobby admin, so `update_table_name` is
callable today. And `init_microstakes_tables` (`src/lobby_canister/src/lib.rs:238`) already does
`tables.clear()` and re-inserts all three records including their names, so nothing new is needed to
rewrite a registered config either, it just re-inserts hardcoded `1_000_000/2_000_000` while
`icp.yaml:66,76` initialises `table_2` at `5_000_000/10_000_000` and `table_3` at
`10_000_000/20_000_000`.

The defect underneath is worse than a stale label and should be re-read as such: **the lobby
advertises stakes the table does not charge.** A player who picks the row labelled
`6-Max - 0.01/0.02` sits down at 0.05/0.10, five times the blind they chose, and at
`9-Max - 0.01/0.02`, ten times. The harness says so on every run:

```
lobby row NAME "6-Max - 0.01/0.02" quotes a small blind: DISAGREES:
  screen "0.01" … canister says 5000000 e8s (screen is 0.200x the chain)
lobby row NAME "9-Max - 0.01/0.02" quotes a big blind: DISAGREES:
  screen "0.02" … canister says 20000000 e8s (screen is 0.100x the chain)
```

This pass did **not** make the calls. They mutate state on a replica four other agents are
photographing against, the table names are read live by every scene, and the durable fix is six
constants in `init_microstakes_tables` plus a lobby redeploy, which is an engine change this pass is
not permitted to make. It is ranked in [WAVE-05.md](WAVE-05.md) for wave 6, re-severitised from
"stale registry" to **high: the lobby misprices every table it lists**.

<a id="h-41"></a>
### H-41, medium, the deposit scene went UNVERIFIED because the copy grew and the assertion did not — STATUS: FIXED (wave 7)

**Status: FIXED.** Both deposit shots were being filed as `UNVERIFIED-deposit-*.png` on every run,
for two reasons that had nothing to do with the deposit flow:

```
TOKEN CENSUS FAILED: 2 of 26 numeric tokens on screen are asserted by nothing , 
  "18"     in div.modal-content > div.modal-body > p.player-notice
  "0.0004" in div.modal-body > div.form-section > div.minimum-notice > span
```

1. **`"18"` is the `18+ only` of a player-protection notice** being read as an unasserted money
   figure. `.player-notice` and `.modal-notices` are the protected copy restated inside a dialog,
   which wave 5 had to do because a 72%-black scrim hides the banner behind it
   ([T-31](#t-31), [H-36](#h-36), [T-36](#t-36)). The `protected-disclaimer-copy` allowlist rule
   named `.alpha-warning-banner, .footer-disclaimer, .disclaimer-content, .legal` and not the two
   in-dialog carriers. Fixed by adding them to that rule, which is the rule's own stated purpose.
2. **`"0.0004"` is a real money figure that nothing asserted.** T-30 added
   *"…charged twice by the ledger, so you need 0.0004 ICP in your wallet to deposit the minimum"* to
   `.minimum-notice`; `assertDepositAgreement` scraped `nums[0]` and `nums[1]` and dropped `nums[2]`,
   and the census rule's label regex named only `Minimum deposit|Network fee`. So the number a player
   with a nearly-empty wallet acts on was ungated. **Allowlisting it would have been the wrong fix**
  , it is money. It is asserted now against `MIN_DEPOSIT + 2 x icrc1_fee()`, both read from their
   sources, and both deposit shots verify at both viewports.

The general shape is worth naming, because it will happen again: **a wave that improves money copy
breaks the census, and the cheap way out is to allowlist the new number.** The census is designed to
make that visible; it only works if the next person asserts instead of allowlists.

### Correction to [H-37](#h-37): paint-order model disagreements are not zero

H-37 records *"0 paint-order model disagreements, no case where the pixels showed coverage the paint
model said was impossible"*, and its self-test prints `ALL 16 PIXEL-GATE CASES PASS` where the entry
says 15 offline cases. Measured on the definitive run of this pass, out of the gate's own manifest:
`table-showdown` desktop reports **1**. The pass's critic measured **2** in a 22-shot sweep and the
gate's own author reported 2. The number is small, non-gating by design and correctly recorded in
the manifest, the defect is that the summary in this file states it as zero.

The mechanism is real and benign: a translucent figure sampling a background through
`backdrop-filter` measures suppression when the background is hidden, with nothing painted on top of
it. The gate reports it as a `paintOrderModelDisagreement` with its fraction rather than failing the
scene, which is right. What is missing is a self-test case that actually reproduces it, the case
named for it asserts only `r.ok && occlusionsFound === 0` and would still pass with the whole
`paintOrderModelDisagreement` path deleted.

<a id="h-42"></a>
### H-42, high, `./scripts/dev.sh test`, the repo's primary gate, can hang forever — STATUS: FIXED (wave 14)

> **FIXED 2026-08-09, and [H-54](#h-54)'s fix was vacuous — measured at HEAD before touching
> anything:**
>
> ```text
> $ out="$(with_timeout 2 sh -c 'sleep 60; echo never')"
> command substitution returned after 60s
> ```
>
> A two-second bound that returns in sixty seconds is not a bound. H-54 killed the WATCHDOG's
> children and never killed the BOUNDED COMMAND's: `kill -9 "$pid"` reaches exactly one process —
> the subshell, which bash has usually exec'd into `sh` — and everything `sh` forked (`cargo`, the
> test binary, its PocketIC server) is orphaned to init **still holding the inherited stdout**. The
> consumer then blocks with zero CPU anywhere, which is H-42's symptom word for word. In `cmd_test`
> that runaway is a whole settlement suite, which is why the original hang was 33 minutes.
>
> **After:** `with_timeout` walks the process table and kills the whole descendant tree, deepest
> first, twice — once when the bound expires and once after the wait, because a runaway can fork
> between the survey and the kill — and returns 124 so a killed command can never look like a pass.
>
> ```text
> 1. bounded command overruns, read through a pipe   -> returned after 3s, rc=124
> 2. deeper tree: sh -> sh -> sleep 90                -> returned after 2s, no surviving sleep
> 3. command finishes inside its bound                -> rc=0, and rc=7 stays 7
> ```
>
> **And the step this entry blamed now HAS a bound, as does every other step.** All nine steps of
> `cmd_test` run through `timed_step`, which prints the measured time next to the bound — because
> the thing H-42 could not say was *which step*. The bounds are >=2.2x the measured wall clock, and
> the measurements are recorded next to them in `scripts/dev.sh` — from the run that turned this
> gate green on 2026-08-09:
>
> ```text
> [1/9] suite wiring              8 s (bound 120)    [6/9] settlement oracle    101 s (bound 900)
>       candid bindings           1 s (bound 300)          pinned reproducers    13 s (bound 300)
> [2/9] cargo test --workspace   24 s (bound 900)    [7/9] shots self-tests       4 s (bound 600)
> [3/9] wasm build (warm)         0 s (bound 900)    [8/9] archive self-tests    88 s (bound 600)
> [4/9] differential              3 s (bound 900)    [9/9] no-peeking harness    42 s (bound 900)
> [5/9] money-safety           1610 s (bound 3600)
>
> ==> result   all fast gates green        rc 0, 1,894 s total
>              money-safety: all 22 cargo targets green
> ```

> **WAVE 12: THE MECHANISM IS [H-54](#h-54), AND IT IS THE OPPOSITE STEP.** The diagnosis below —
> "step 4 has no time bound, step 5 does" — is the wrong way round. Step 5's `with_timeout`
> watchdog forks a `sleep`, `kill "$wd"` kills only the subshell, and the orphaned `sleep` keeps
> the gate's **stdout and stderr** open at PPID 1. The gate exits; anything piping its output then
> blocks for the rest of the bound with zero CPU everywhere. `900 + 300` seconds is the twenty
> minutes, and every suite has already passed or failed by then. Measured and fixed in
> [H-54](#h-54). **This entry stays OPEN**: step 4 genuinely has no time bound, which is still
> worth fixing, and a real hang inside it would look the same from outside.

**Status: executed, new in this pass.** `cmd_test` step 4 runs the money-safety targets with **no
time bound**, while step 5 wraps the settlement targets in one:

```sh
# scripts/dev.sh:453-458 , money-safety, UNBOUNDED
cargo test --test invariants  -- --test-threads=2 &&
cargo test --test regressions -- --test-threads=2 &&
…
# scripts/dev.sh:504-506 , settlement, bounded
with_timeout 900 sh -c 'cd tests/settlement && cargo test --test settlement …'
with_timeout 300 sh -c 'cd tests/settlement && cargo test --test disagreements …'
```

Observed here. `./scripts/dev.sh test` reached step 4, emitted 22 `test result:` lines across the
earlier targets, entered `cargo test --test invariants -- --test-threads=2`, and then stopped for
**33 minutes and counting** with no further output and no test line. Both ends of the run were
idle, not slow:

```
$ ps -o pid,time,%cpu -p <invariants binary>       # sampled 45 s apart, twice
  48927   0:05.50   0.0        …unchanged…
$ ps -o pid,time,%cpu -p <its own pocket-ic>
  48977   0:53.52   0.0
$ lsof -p 48927 -a -i
  invariant 48927 josh 14u IPv4 TCP localhost:57995->localhost:57993 (ESTABLISHED)
```

A live socket to a live PocketIC server with **zero CPU on both sides** is a blocked wait, not
progress. Four other PocketIC servers belonging to concurrent agents were running on the machine, so
contention is a plausible trigger and this may not be deterministic, which makes it worse, not
better: a gate that hangs sometimes is a gate people learn to skip.

This is [H-22](#h-22)'s exact shape one directory over. H-22 records the settlement suite hanging
rather than failing and says it is *"contained by a timeout in `cmd_test`, not fixed"*: the
containment was applied to `tests/settlement` and never to `tests/money_safety`, which is where the
fund-safety invariants live.

**Fix:** wrap step 4 the way step 5 already is. A one-line change per target, and the bound should
be generous (the suite legitimately takes many minutes under load) but finite. Separately, a
`--test-threads=2` money suite that spawns its own PocketIC per test is a poor citizen on a shared
machine; step 4 could reasonably run single-threaded.

<a id="h-43"></a>
<a id="h-44"></a>
### H-44 — medium — the fuzz generator cannot reach the sequence M11 exists to catch — STATUS: OPEN

**Status** measured 2026-08-05, while proving M11 OUTCOME goes red on the defect it was written
for.

**What was measured.** With `is_in_hand`'s cardless disjunct deliberately restored (the
docs/SECURITY-FINDINGS.md FINDING 17 defect, live), a full default fuzz run — three seeds, 220
steps each — reported **0 blocking findings**. M11's structural leg ran on 92 steps of the seed
that had previously found E-36 and stayed silent, because the generator never produced the
sequence:

```
money-fuzz: seed 0xc1ea2dec0002 finished: 6 hands, 0 blocking finding(s)
money-fuzz: seed 0xc1ea2dec0002 M11 OUTCOME: 6 hand(s) had their OUTCOME checked (of 6 the
            watch saw); the structural leg ran on 92 step(s)
```

The same M11 code, driven over the scripted sequence against the same module, fires immediately:

```
a_live_hand_may_not_run_on_with_one_claimant: hand #1: the hand is STILL LIVE with 52000000
e8s in it and exactly ONE seat holding a claim (seat 1, lpoz5-…)
```

**Why.** The sequence needs four things to line up: a hand LIVE, a chair EMPTY (somebody left
mid-hand), `join_table` on that exact seat, and `sit_in` after it. `next_op` picks seats at
random from `0..max_players` with a 1-in-6 chance of an out-of-range seat, and weights
`JoinTable` at 4/226 and `SitIn` at 5/226, so the conjunction is rare enough not to appear in
660 steps. E-36 was originally found at **600 steps on one seed**, which is consistent with rare
rather than impossible.

**What this means for the gate.** M11 is real — it convicts the defect on the real canister
through `m11_is_quiet_on_the_finding_17_sequence`, which IS in the fast gate — but the class is
currently gated by a SCRIPTED probe rather than by search. That is the same shape as
[H-27](#h-27): a defect class whose reproducer no generator aims at.

**Fix.** Give the generator a compound op that takes an empty chair mid-hand and sits in — the
generator already knows which seats are occupied is not something it tracks, so the cheap version
is `Op::TakeAnEmptyChairMidHand { actor }` applied by `actions::apply`, which can read the state.
Not taken here: `next_op`'s weights are another owner's surface and changing them changes what
every existing seed explores.

### H-43, medium, a partial screenshot run silently destroys the full run's index and manifest — STATUS: OPEN

**Status: executed, new in this pass, by doing it accidentally and having to redo a 25-minute run.**

`writeManifest` and `writeIndex` (`tools/shots/lib/capture.mjs:127-134`) unconditionally overwrite
`<sha>/manifest.json`, `<sha>/INDEX.md` and both mirrors in `latest/` with the manifest of **the
scenes this invocation ran**. So:

```
$ ./scripts/dev.sh shots                                   # 22 shots, manifest 5,972 KiB
$ node tools/shots/run.mjs --scenes lobby --viewports desktop --skip-build --skip-deploy
$ python3 -c "…json.load(open('artifacts/screens/fe72d46/manifest.json'))…"
scenes in manifest: ['lobby']                              # manifest now 363 KiB
```

The 72 PNGs survive, they are keyed by filename, but **the index and the manifest that say what
they prove do not**, and nothing warns. A reader who opens `INDEX.md` after any partial run sees one
row and has no way to tell whether the other twenty-one shots were never taken, taken and failed, or
taken and verified an hour earlier.

`run.mjs` already understands this hazard for one artifact type and handles it well: a full run wipes
`latest/` up front, a partial run does not, and `clearLatestVariants` retires only the filenames the
current (scene, viewport) could claim, *"because a verified PNG left over from an earlier commit is
exactly the artifact a reader would trust"*. The same reasoning applies with more force to the
manifest, which is the only machine-readable record of every gate's verdict.

**Fix:** on a partial run, read the existing manifest and merge scene entries by `(scene, viewport)`
rather than replacing the file, or, at minimum, write partial runs to
`manifest-partial-<scenes>.json` and leave the full one alone. Either is a small change in
`capture.mjs`.

---

<a id="e-52"></a>
### E-52, high, the app's own error toast paints over the player-protection notices at portrait — STATUS: FIXED (wave 7)

**Status: FIXED IN WAVE 7, and the class now has a gate.** What follows is the wave-6
measurement, unchanged, with the fix and its gate recorded at the end.

**Status when found: executed in wave 6, at both viewports, deterministically.** HARD RULE 2 says the
unaudited-alpha disclaimer, the 18+ notice, the jurisdiction warning and the no-rake property must
be **on screen, not merely in the DOM, at any viewport and on any view**. They are not, whenever the
app is showing an error.

`.toast` is `position: fixed; z-index: 100`. The alpha-warning banner is `z-index: 100` too, and
comes first in paint order, so the toast wins. Measured with every agent call failed at the network
layer, so the toast is raised by the app's OWN error path and not injected:

```
### portrait 390x844  toasts=1
   toast toast.error rect=[-117, 80, 624, 534] z=100 pos=fixed "Failed to fetch HTTP request: …"
   COVERED  "Unaudited code with known bugs"            p.banner-warning at y=16  toastHits=3/9
   COVERED  "your funds are NOT safe"                   p.banner-warning at y=16  toastHits=3/9
   COVERED  "illegal in many jurisdictions"             p.banner-warning at y=16  toastHits=3/9
   COVERED  "18+ only"                                  p.banner-warning at y=16  toastHits=3/9
   COVERED  "No rake is taken from any pot on any table" strong at y=135          toastHits=9/9
                                                        (the only other carrier is offscreen)

### desktop 1440x900  toasts=1
   toast toast.error rect=[360, 75, 720, 534] z=100 pos=fixed
   readable  … all five: the banner is 1440 px wide, so the 720 px toast leaves the text clear,
             and the no-rake property has two more carriers at y=700 and y=843
```

At 390 px the toast is **624 px wide starting at x = −117**: it overflows the viewport on both
sides, so there is no clear column at all. Its top edge is y = 80 and the no-rake line is at
y = 135, so **any** toast taller than 55 px covers it; 534 px is merely what a long error message
produces.

**Why no gate saw it.** `tools/shots/lib/protected-notices.mjs` is a good probe and hit-tests each
phrase on its own pixels — but no scene in `tools/shots/scenarios` raises a toast, and
`lib/occlusion.mjs` is written against `.modal-backdrop`. `make hygiene` greps the source. The
phrases are present, unoccluded and legible in every state the harness puts the app into; the app
just has a state the harness never puts it into. This is the wave-4 lesson recurring one layer out:
the notices were measured on the rendered page, and the *set of rendered pages* was the blind spot.

**Reproduce**
```
node $SCRATCH/toast2.mjs http://<frontend-canister-id>.localhost:8077
```
(aborts `**/api/v2/**` and `**/api/v3/**`, clicks the app's own dev sign-in, waits 4 s, then
hit-tests all five phrases against `.toast` at both viewports.)

**THE FIX (wave 7), three locks in `src/cleardeck_frontend/src/routes/+page.svelte`:**

1. **Position.** `.app` publishes the measured height of `.alpha-warning-banner` as
   `--notice-safe-top` (one `bind:clientHeight`), and `.toast` starts at
   `calc(var(--notice-safe-top) + 12px)`. The banner is the first element in the flow, so at
   scroll offset *s* it occupies viewport rows `[-s, H-s]` while the toast starts at `H+12`.
   `H+12 > H-s` for every `s >= 0`, so they cannot overlap at any scroll position and no
   scroll listener is needed. Scrolling only widens the gap.
2. **Paint order.** `.toast` moved from `z-index: 100` to `90`, below the banner (100) and
   below `footer` (raised from 10 to 95, because `.footer-disclaimer` is the copy a DESKTOP
   player reads once the banner has scrolled away). So even a wrong measurement cannot win
   the `elementFromPoint` hit test the notice probe runs.
3. **Size.** `max-width: min(560px, calc(100vw - 24px))` and `max-height: min(40vh, 320px)`
   with `overflow-y: auto`. The 624 px-wide, 534 px-tall box is now impossible whatever the
   message says; long text wraps (`overflow-wrap: anywhere`) and then scrolls inside.

**THE GATE, which is the part that matters.** A z-index is one line and one line can be
edited away, so:

* `tools/shots/lib/toast-notices.mjs` raises a toast and re-runs the protected-notice probe
  with it up. It asserts all three locks separately — the hit test, the box fitting inside
  the viewport, and `toast.top >= banner.bottom` — so the locks cannot silently collapse into
  one.
* `tools/shots/run.mjs` runs it **centrally, for every scene at every viewport**, exactly
  where the census, the occlusion gate and the notice probe run. There is no per-scene
  opt-in and no opt-out. That is the answer to "no scene raises a toast": every scene now
  does.
* The toast it raises carries the component's own Svelte scope class and is required to
  `matches()` one of the `.toast` rules in the live CSSOM, so it is painted by the app's own
  rule rather than by a lookalike.
* The `toast-notices` scenario raises a **real** toast through the app's own error path
  (every canister call aborted at the network layer, then a reload, so `loadTables()`'s own
  catch assigns `error = e.message`) and compares its computed style and geometry against the
  injected one field by field. If they ever disagree, the central gate is measuring something
  the player never sees, and that scene goes red and says so.

**MEASURED ON THE REAL TOAST, both viewports, after the fix.** Same message shape as the
wave-6 measurement — the agent's own "Failed to fetch HTTP request" text — raised the same way:

```
portrait 390x844
   toast  rect=[12, 297, 366, 320]   z=90 pos=fixed   banner bottom y=286
   5 of 5 protected notices ON SCREEN and unoccluded          (wave 6: 0 of 5 usable)
desktop 1440x900
   toast  rect=[440, 189, 560, 320]  z=90 pos=fixed   banner bottom y=178
   5 of 5 protected notices ON SCREEN and unoccluded
```

Compare the wave-6 shape at the same viewport: `rect=[-117, 80, 624, 534] z=100`. The box now
sits inside a 390 px screen instead of overhanging it by 117 px on the left and 234 px on the
right, and it starts 11 px BELOW the banner instead of 206 px inside it. The injected toast
the central gate uses measured `x=12, top=297px` at portrait and `x=440, top=189px` at
desktop — the same left edge and the same top as the real one, matching on `position`,
`z-index`, `top`, `max-width`, `max-height` and `overflow-y`, and painted by the app's own
compiled rules `.toast.svelte-1uha8ag` and `.toast.error.svelte-1uha8ag`.

**Two false reds the gate produced on its first full run, and what they cost.** Neither was a
notice being covered; both were the gate being wrong, and both are worth recording because a
gate that cries wolf is the one people switch off:

1. `shuffleproof` at portrait scrolls the proof panel into shot, which leaves the banner at
   `y = -90`. The toast pass reported all five notices off screen. `run.mjs` already answers
   this for the toast-free probe — measure where the scene left the page, and if anything is
   off screen re-measure at scroll 0 and let that be the verdict — and the toast pass now does
   the same. A notice a toast covers is covered at scroll 0 too.
2. `toast-notices` failed `assertPageHealthy`. Its console line was
   `[ERROR] Failed to load tables: TransportError: ... TypeError: Failed to fetch` — the app
   CATCHING the failure and logging it, with `pageErrors` empty. `UNCAUGHT_CONSOLE` matches
   `/\b(Type|Reference|…)Error\b/` anywhere in the text, so a handled failure whose message
   quotes a `TypeError` was classified as an escaped throw. `page-health.mjs` now exposes
   `forgetConsoleFaults(page, pattern, reason)`, used by that one scene, which refuses to
   forgive anything in `pageErrors` and records the forgiven lines in the manifest.

---

<a id="e-53"></a>
### E-53, medium, the screenshot harness hardcodes a controller identity that does not control anything here — STATUS: FIXED (wave 7)

**Status: FIXED IN WAVE 7. There were THREE instances, not one, and the third one was found
by running the fix.** The wave-6 write-up follows unchanged; the fix and the third instance
are recorded at the end.

**Status when found: executed in wave 6.** `./scripts/dev.sh shots` aborts:

```
FATAL: No local icp identity controls 46el7-ql777-77775-aaada-cai.
  controllers on the canister: nsp5q-aelxk-xax2u-3sqxb-xeod6-fduhf-zmoir-qxpp3-xusb4-nez2q-5ae
  identities tried: cd-local-deployer=sg3sw-…, cd-alice=…, cd-bob=…, cd-carol=…, (default)=…
Controller-only calls (reset_table) cannot be made, so scenes cannot be reset.
```

`nsp5q-…` is `cyclepay-hotwallet`, which is not in `FUNDER_IDENTITIES`. This is the SAME defect the
wave-6 history-wiring agent found and fixed in `scripts/dev.sh` — `resolve_controller_identity` now
reads the controller off `canister status` instead of assuming `cd-local-deployer` — applied to a
second harness that was not looked at. The consequence is not cosmetic: **the pixel gate, the
occlusion gate and the protected-notice gate all live behind `shots`, so a stale identity list
disables three fund-adjacent gates at once and reports it as a fatal setup error rather than a red
gate.**

The lobby half of the same class also survived the wave-6 dev.sh fix: after the documented bring-up
`get_tables` returned `(vec {})`, and the wave-6 reconciler had to call
`init_microstakes_tables` by hand as `cyclepay-hotwallet` before any table view could be reached at
all. `icp canister call` exits 0 on a Candid `Err`, so `up_wire`'s `|| warn` never fires.

**And an aborted run destroys the artifact index before it captures anything.** `run.mjs` wipes
`artifacts/screens/latest/` up front on a full run — correct, because *"a verified PNG left over
from an earlier commit is exactly the artifact a reader would trust"* — but it wipes it in step
`[6/6]`'s preamble, BEFORE the first scene, and the identity check that kills the run fires inside
the scenes. So a run that produces nothing still deletes `INDEX.md`, `manifest.json` and every PNG
of the previous run. The wave-6 reconciler hit this and rebuilt `latest/` by hand from
`artifacts/screens/fe72d46/` (the last full run) with `artifacts/screens/fcfa4e8/` (a later partial)
layered on top. This is the same class as the wave-5 note on partial runs clobbering the manifest,
one step worse: there, a partial run leaves a misleading index; here, a failed run leaves none.

**Fix:** port `resolve_controller_identity` from `scripts/dev.sh` into `tools/shots/lib/ids.mjs`
(its doc comment already says the hardcode "is an assumption"), make `up_wire` fail rather than
warn when `get_tables` comes back empty, and move the `latest/` wipe to AFTER the first scene
verifies, or write into a temporary directory and swap it in on success.

**THE FIX (wave 7), and the third instance.**

`resolveControllerIdentity` was already in `tools/shots/lib/ids.mjs` and was already being
called — the port had happened, and it still did not work, because it tried a **four-name
allowlist plus whatever identity happened to be selected**. `scripts/dev.sh` enumerates
`icp identity list`; the JavaScript did not. So:

1. `lib/ids.mjs` now enumerates every identity `icp identity list` knows about, reading each
   principal from the same command's output rather than spawning a subprocess per name. The
   failure message names how many identities were tried instead of listing four.
2. **The third instance, found by running the fix.** `./scripts/dev.sh local-up` died in step
   `[6/6]` with `IC0512 Only controllers of canister 5uljf-… can call ic00 method
   update_settings`, *after* a successful backend deploy. `lib/frontend-build.mjs`'s
   `deployFrontend()` ran `icp deploy -e local frontend` with no identity at all. On this
   machine that is three different controllers for one stack — the tables belong to
   `cyclepay-hotwallet`, the frontend asset canister to `oms-port-trial`, and the docs name
   `cd-local-deployer` — because each canister was created by whatever identity was selected
   at the time. `deployFrontend` now resolves the controller off the canister the same way,
   and skips the lookup when the canister does not exist yet (a fresh machine has no
   controller list to read, and the deploy is what creates it).
3. The `latest/` wipe is now **lazy**. `run.mjs` hands a `claimLatestDir` callback to the
   scene runner, and it fires from `writeShot`, one statement before the first PNG is
   written — after the replica, the identities, the deploy, the staging and the verification
   of one whole scene have all worked. `clearLatestVariants` moved to the same place for the
   same reason: it used to run before `scene.setup`, so a scene that failed to stage deleted
   the previous run's PNG and put nothing in its place.

**Verified by running it**: `node tools/shots/run.mjs` now completes the full sweep on this
machine, resolving `cyclepay-hotwallet` for the tables and `oms-port-trial` for the frontend,
with no identity abort anywhere.

**What is NOT fixed here**, because it is another owner's file: `up_wire`'s lobby half still
uses `|| warn` on `set_admin` and `init_microstakes_tables`, and `icp canister call` still
exits 0 on a Candid `Err`. Its postcondition (`lobby lists N table record(s)`) is printed but
not asserted. Related and also not fixed: the lobby's registered table NAMES quote stakes that
disagree with the table canisters' own configs — `"6-Max - 0.01/0.02"` against a canister
reporting 0.05/0.10, `"9-Max - 0.01/0.02"` against 0.10/0.20 — which the shots harness has
reported as a chain disagreement on every full run since `7bc69db`, three waves ago.

---

<a id="e-58"></a>
### E-58, high, the project's own verifier could not verify the project's own canisters — STATUS: FIXED-NO-GATE (wave 7)

**Status: FIXED IN WAVE 7.** An auditor followed `./scripts/verify-build.sh --local`
literally, as a stranger, after the documented `./scripts/dev.sh local-up`, and got
**NOT VERIFIED, 6 of 6 MISMATCH**, with every canister reporting `git:dirty = dirty`. The
reproducible-build apparatus wave 6 built is real — a vendored recipe with
`--remap-path-prefix`, a digest-pinned image, the git revision as canister metadata — and the
end-to-end story still did not close for the one person it exists for.

**Two independent causes, either of which alone produces six mismatches.**

**1. The two commands used two different compilers.** `local-up` runs `icp deploy`, which
builds with the host toolchain: on this machine an aarch64 macOS `cargo`. `verify-build.sh
--local` rebuilt inside the `linux/amd64` container and compared. Those are never
byte-identical, and the Dockerfile has said so all along — it is the reason the platform is
pinned. Measured on one source snapshot at revision `4af6dc6`:

```
host   (Darwin arm64, cargo 1.90.0)   table_canister -> 76db1037…
docker (linux/amd64,  cargo 1.90.0)   table_canister -> d4979af9…
```

So the verifier was comparing a build of the source against a *different, equally correct*
build of the same source and reporting it as possible tampering. That is the worst failure
mode a verification tool has: it cries fraud at a consistent system, and a reader who sees
that once stops believing it the next time.

**2. The metadata is part of the hash, and the verifier always used its own.**
`git:revision` and `git:dirty` are stamped into the module as public custom sections, so they
are inside the bytes being hashed. The script built with the verifier's HEAD and the
verifier's tree state, so a deployment made one commit earlier — or from a dirty tree, which
every local deployment on a working machine is — mismatched for a reason that had nothing to
do with the code. This was the cause of the `git:dirty = dirty` line in the auditor's output
being read as a symptom rather than as the second half of the bug.

**THE FIX.**

* `scripts/dev.sh local-up` writes `.icp/cache/cleardeck-build-provenance.txt` recording which
  builder produced the installed modules, and `verify-build.sh --local` reads it and rebuilds
  the same way. `--host` / `--docker` override it. `--mainnet`, `--two-paths` and `--emit`
  stay container-only and refuse `--host`: there is one right answer for a published
  deployment and every verifier must get it.
* `local-up --docker` builds through `verify-build.sh --emit` and upgrades every backend
  canister to the container-built module, so a local replica can be a **full rehearsal** of
  the mainnet check rather than an analogy of it.
* The host build runs in a scratch tree outside the repository, so it cannot disturb
  `.icp/cache/artifacts` (which is the deployment other agents are using) and so every run
  re-exercises path independence for free.
* The verifier reads `git:revision` and `git:dirty` **off each canister** and builds with
  exactly those values, grouping the fleet by distinct label so a mixed-revision fleet is
  rebuilt once per revision instead of mismatching wholesale. CODE and LABEL are then reported
  as two separate facts. A stale label can no longer masquerade as tampering, and it cannot
  hide tampering either: the label only chooses which source to compare, and a canister cannot
  claim a revision whose build produces its hash.
* Every run ends with an explicit statement of what it does and does not establish, keyed to
  the builder it used. A `VERIFIED` with no scope is how "the code running is the code in this
  repo" became a claim nobody could check.

**Measured, both paths, on this machine, with the source proven unchanged across each pair**
(three other agents were editing `src/table_canister/src/lib.rs` during this wave, which is
itself why the digest is taken before and after):

```
./scripts/dev.sh local-up && ./scripts/verify-build.sh --local
  builder host   6 of 6 MATCH   VERIFIED

./scripts/dev.sh local-up --docker && ./scripts/verify-build.sh --local
  builder docker 6 of 6 MATCH   VERIFIED
```

**What this still does not establish**, and the README now says so in a table: the **mainnet**
fleet predates this pipeline and carries no `git:revision` at all, so it remains unverifiable
until it is redeployed from `--emit` output. Verification also says nothing about whether the
code is correct. A `VERIFIED` on a canister full of known defects verifies the defects.

---

<a id="e-54"></a>
### E-54, high, nothing on chain moved the game — STATUS: FIXED (wave 7)

**Status: executed, before and after, on modules built from this tree.**
Reproducer and gates: `tests/money_safety/tests/timers.rs`.

```text
cd tests/money_safety && cargo test --test timers -- --nocapture --test-threads=1
```

#### What it was

`ic-cdk-timers = "1"` was declared at `src/table_canister/Cargo.toml:22` and `set_timer`
appeared **nowhere** in `src/`. Every clock in the engine — the action clock, the disconnect
threshold, the sitting-out kick, the reload timer, the stuck-hand grace — was evaluated only
inside `check_timeouts`, an ordinary update call. A table whose clients all closed their tabs
froze.

#### The dead window, measured before the fix

Heads-up, two funded seats, a real 3,000,000 e8 pot, then every client closes. Nothing after
that point sends the canister a message; only `pic.tick()` (the subnet producing blocks) and
queries (which cannot change state):

```text
  t+  60s  phase=PreFlop pot=3000000 stuck=false abandonable_in=Some(269)s seated=2
  t+ 300s  phase=PreFlop pot=3000000 stuck=false abandonable_in=Some(29)s  seated=2
  t+ 360s  phase=PreFlop pot=3000000 stuck=true  abandonable_in=None       seated=2
  t+1200s  phase=PreFlop pot=3000000 stuck=true  abandonable_in=None       seated=2

  RESULT: hand resolved at None, seats released at None
      seat0 chips=198000000 bet=2000000 folded=false cards=true status=Active
      seat1 chips=199000000 bet=1000000 folded=false cards=true status=Active
```

**The dead window was not 5.5 minutes. It was unbounded.** The auditor's
`abandonable_in_ns = 169_194_575_000` is the time until `abandon_stuck_hand` would be
*accepted*, and that method needs a caller too. Note also that after twenty minutes of total
silence both seats were still marked `Active`: the disconnect threshold had never been
evaluated either, because nothing evaluated anything.

#### The dead window, measured after the fix

Identical setup, identical silence, same harness:

```text
  t+  30s  HAND RESOLVED ITSELF, phase=HandComplete
      seat0 chips=201000000 folded=false   <- won the pot on the fold-out
      seat1 chips=199000000 folded=true    <- folded by its own clock
  t+ 210s  every seat released, chips back in escrow
  internal total unchanged at 4000000000 e8s
```

* **30 s** to resolve the hand — exactly the table's action clock.
* **210 s** to release every seat — exactly `DISCONNECT_TIMEOUT_SECS` (90) +
  `SITTING_OUT_KICK_SECS` (120). At that point the chips are in escrow and reachable by an
  ordinary `withdraw`.
* Zero e8s created or destroyed on the way.

A mid-hand upgrade is gated separately (`the_clock_survives_a_mid_hand_upgrade`): after
`install_code --mode upgrade` the canister reports `clock_watchdog_armed=true`,
`next_wake_at` 29 s out — aimed at the *restored* action clock — and the hand still resolves
itself in 30 s with no client attached.

#### The shape, and the fact that decided it

A repeating watchdog interval (30 s) plus a one-shot wake aimed at the exact next deadline.
The deciding fact is in `ic-cdk-timers` 1.0 itself, `global_timer.rs::do_timer` step 7:

> If a repeated timer is successfully **dispatched** (irrespective of the timer's own success),
> reschedule it.

A **repeating** timer is rescheduled *before* its callback runs, so it survives a callback that
traps. A **self-rescheduling one-shot** does not: the re-arm lives in the same message as the
work, so one trap and the clock is gone permanently, with nothing on chain to notice. On a
canister whose settlement path has trapped before ([E-42](#e-42)/FINDING 15), that difference
decides the design. This was not a theoretical concern — see E-56 below.

#### Cost, measured

Empty table, one simulated hour, PocketIC 13-node application subnet:

| watchdog | ticks/hour | cycles burned | cycles per tick |
|---|---|---|---|
| 30 s | 120 | 1,843,363,200 | **15,361,360** |
| 60 s | 60 | 923,160,120 | **15,386,002** |

Exactly linear in ticks, so the per-tick figure prices every alternative:

| shape | idle burn/day | idle burn/year | worst-case lateness |
|---|---|---|---|
| `set_timer_interval(1s)` | 1.328 T | **485 T** | 1 s |
| `set_timer_interval(10s)` | 0.133 T | 48.5 T | 10 s |
| **watchdog 30 s + one-shot (chosen)** | **0.0442 T** | **16.2 T** | ~0 while a hand runs |
| watchdog 60 s + one-shot | 0.0222 T | 8.1 T | ~0 while a hand runs |

The bare 1 s interval the finding suggests costs 485 T/year for a table nobody is sitting at
and is *less* precise than the hybrid, because the one-shot is aimed at the actual expiry
rather than at a fixed grid. A tick is not one message: the crate's trap-catching self-call
makes it a `canister_global_timer` execution, an inter-canister call, the `timer_executor`
update and a reply callback.

`CLOCK_WATCHDOG_SECS` is the single knob: burn is `86400 / period * 15.4M` cycles per day.

#### What the clock deliberately does NOT do

* **It does not deal hands.** `advance_table_clock` still only *reports* `AutoDealReady`.
  Dealing to seats whose clients are gone would post their blinds hand after hand, which is a
  way to lose money to a timer rather than to a player.
* **It does not abandon a live hand that has no action clock at all.** `hand_is_stuck` treats
  that as stuck and `abandon_stuck_hand` still refunds it on request; `clock_should_abandon`,
  which is what the clock uses, does not. See that function's comment: a human calling
  `abandon_stuck_hand` is a decision, a timer doing it every 30 s on a predicate whose truth
  conditions are not fully understood is a policy, and that branch has not earned one.
* **It does not void a hand just because the wall clock says five minutes.** The stuck-hand
  grace is measured from **when the canister first SAW the clock overdue**, not from the
  timer's own `expires_at`, so a stall in which the canister does not execute buys no credit
  toward abandonment. This is the E-56 defect below and it has its own gate,
  `a_hand_stalled_past_its_grace_is_played_out_not_voided`.

---

<a id="e-55"></a>
### E-55, high, cycles: with ten tabs open the runway was DAYS, not months, and still nothing tops it up — STATUS: OPEN (measurement and monitoring closed, funding open)

**Status: OPEN, and the reason it is still open is stated exactly.** Three of the four things
this entry asked for exist now and each names a gate. The fourth — *a funding path* — is a
DESIGN below, not a mechanism in the tree, because the mechanism needs money and a decision
that is not an engineer's to make. Until somebody funds it, the honest answer to *"can I
always get my money out"* is still **no**, for a reason that has nothing to do with poker.

An IC canister below its freezing threshold **rejects every update call**. On this canister
that is `deposit`, `withdraw`, `cash_out`, `player_action`, `reload` and `abandon_stuck_hand`
failing simultaneously: every player at the table unable to reach their own money at the same
moment, with no attacker involved and no in-application remedy.

Two facts about that state, established rather than assumed, in
`tests/money_safety/tests/cycles_runway.rs::a_frozen_table_answers_nothing_and_is_fully_recoverable`:

* **It is fully recoverable.** A top-up restores every method and every balance to the e8. The
  test freezes a table holding a player's 10.00000000 ICP, proves nothing answers, restores the
  threshold and withdraws successfully. Freezing destroys nothing.
* **Running to TRUE ZERO is not** — and this half is **NOT measured here.** The IC uninstalls a
  canister that reaches zero and its state is gone; that is the platform's documented behaviour
  and nothing in this tree drives a canister to zero to confirm it. It is stated because it is
  the difference the alarm thresholds are sized around, and it is labelled because the
  difference between "we froze a table and unfroze it" and "we destroyed one" is exactly the
  kind of thing this register has been wrong about before.

#### 1. THE RUNWAY, MEASURED UNDER LOAD. The old table was for a table nobody is using

The figures this entry used to carry were measured on an **EMPTY** table: the on-chain clock
and nothing else. Every table in a lobby that anybody is actually playing at costs more, and
the difference is not marginal. Measured on the module in this tree, PocketIC 13-node
application subnet, every figure a difference of two `pic.cycle_balance` reads taken from
OUTSIDE the canister:

| unit | cycles | how |
|---|---|---|
| one hand, net of the clock | **168,327,574** | 10 hands, 3 funded players, real post-flop betting, all to showdown |
| one hand, raw | 170,376,082 | the same window, undivided |
| a deposit | **12,664,287** | `icrc2_approve` + `deposit` + the ledger round trip |
| a withdrawal | **12,705,850** | `withdraw` + `icrc1_transfer` round trip |
| **a heartbeat** | **7,156,658** | and see the next section, because this is the whole bill |
| a heartbeat from a principal NOT at the table | 6,829,064 | **REFUSED, and still charged.** Rate limiting moves the price, it does not remove it |
| a query | **0** | measured first, because every figure above is a difference taken while the harness was watching |

Turned into days, at the balances an operator holds.

**CORRECTED IN WAVE 14 ([E-92](#e-92)). THE ROWS WITH TABS IN THEM WERE WRONG, AND THEY WERE WRONG
IN THE DIRECTION THAT MAKES A TABLE LOOK SAFER THAN IT IS.** They priced an open browser tab as a
10-second heartbeat stream and left out the 500 ms `check_timeouts` UPDATE poll, which measured
**1.0841 T/day per tab against the heartbeat's 0.0539** — about twenty times larger. The rows below are rebuilt from `tools/cycles/burn-table.json`,
which is generated from measurements taken against a real canister on the local replica with a
no-tabs control (`tools/cycles/tab-burn.mjs`; the control was taken twice, thirty minutes apart,
and agreed to 0.011%). The per-hand, per-deposit and per-withdrawal costs in the table above are
carried forward from this suite unchanged.

| what the table is doing | burn/day | 1 T | 10 T | 51.4 T |
|---|---|---|---|---|
| idle, nobody at it | 0.0411 T | 24 d | **243 d** | 1,249 d |
| 500 hands/day, no tabs open | 0.1253 T | 7 d | 79 d | 410 d |
| 500 hands/day, **6 tabs open** (typical) | 0.4685 T | 2 d | 21 d | 109 d |
| 500 hands/day, **10 tabs open** (typical) | 0.6974 T | 1 d | 14 d | 73 d |
| 500 hands/day, **6 tabs open** (client's ceiling) | 1.2825 T | 0 d | 7 d | 40 d |
| 500 hands/day, **10 tabs open** (client's ceiling) | 2.0539 T | 0 d | **4 d** | 25 d |
| 500 hands/day, **6 tabs open** — *before* [E-92](#e-92) | 6.9531 T | 0 d | 1 d | 7 d |
| 500 hands/day, **10 tabs open** — *before* [E-92](#e-92) | **11.5050 T** | 0 d | **0 d** | **4 d** |

Two per-tab prices are published rather than one because either alone is a lie: "typical" is the
fixed client measured against a table nobody is playing at, "ceiling" is its rate limiter pinned
wide open all day. The unknown-burn floor the CI monitor alarms on is built from the **ceiling**,
because an alarm threshold is the one place the pessimistic figure is the right one.

**Before wave 14, ten open browser tabs on a table dealing 500 hands a day had ZERO days on 10 T
and four days on the 51 T these canisters actually hold** — with nobody hostile, nobody flooding,
and every player behaving normally. The freezing reserve, nominally thirty days, was worth
**four minutes** at that rate. After the fix the same table has 4 to 14 days on 10 T depending on
how hard it is being driven. **It is still days, not months, and still nothing tops it up.**

Reproduce the burn measurement with `./tools/cycles/run-matrix.sh && node tools/cycles/build-burn-table.mjs`.
Reproduce the per-unit costs with
`cd tests/money_safety && cargo test --test cycles_runway -- --nocapture --test-threads=1`.

#### 2. The largest single cost was the frontend's POLL, not its heartbeat (corrected in wave 14)

`src/cleardeck_frontend/src/routes/+page.svelte` sends `heartbeat()` **every 10 seconds** for
every open tab at the table. That is 8,640 permissionless update calls per player per day, each
an ingress message the IC charges to the canister:

```
  0.0618 T/day PER SEATED PLAYER
  0.3710 T/day for a full 6-max table of open tabs, before a single card is dealt
```

Against an idle burn of 0.0442 T/day, six open browser tabs cost more than eight times what
the on-chain clock costs, and the clock is what this project spent a whole wave worrying
about.

**THAT WAS THE SECOND-LARGEST COST, NOT THE LARGEST, AND WAVE 13 STOPPED COUNTING TOO SOON
([E-92](#e-92)).** The same page also drove `check_timeouts` — an UPDATE call, no `query` in the
`.did` — from a **500 ms** `setInterval`. Measured against a no-tabs control on the local replica:
**1.1380 T/day PER OPEN TAB, against the heartbeat's 0.0618.** Ten tabs on one table burned
**9.7618 T/day**. The heartbeat was 5% of the bill and it was the only part anybody had priced.

`HEARTBEAT_INTERVAL` was not the knob that mattered either. `POLL_INTERVAL` was, and the fix was
not to slow it down — it is a render rate and it should be fast — but to stop it sending an update
at all. See [E-92](#e-92).

**And it is per open TAB, not per seated player.** `sendHeartbeat` short-circuits only on
`!tableActor`, so anybody with the table page open heartbeats — seated or not. The canister
REFUSES a heartbeat from a principal who is not at the table and is charged **6,829,064 cycles**
for the refusal, 95% of what an accepted one costs, because the IC charges ingress induction and
execution before the canister decides to say no. A spectator is therefore almost exactly as
expensive as a player, and no rate limit changes that: `MAX_HEARTBEATS_PER_SECOND` moves the
price of a flood, it does not remove it. That is the same mechanism as
[FINDING 26](SECURITY-FINDINGS.md#finding-26), reachable by an ordinary browser tab rather than
by an attacker. Not a defect on its own — the heartbeat is what the disconnect threshold reads — but it
is the dial an operator would turn first, it is in a file this project owns, and no document
here had ever multiplied it out. `CLOCK_WATCHDOG_SECS` is not the knob that matters — and
neither, it turned out, is `HEARTBEAT_INTERVAL`. `POLL_INTERVAL` was, by about twenty times, and it
was invisible to this analysis because it was a call nobody had thought to look up in the `.did`
([E-92](#e-92)).

This also re-prices [FINDING 26](SECURITY-FINDINGS.md#finding-26). Its 65x amplification was
measured against an idle baseline. Against a table with players at it the *multiple* is smaller
and the *absolute* burn is worse, and the attacker still pays nothing.

#### 3. `get_cycle_status` READ HIGH, which is the one direction that matters — FIXED

`CYCLES_ORIGIN` was anchored at the first tick of the instance and moved only on a top-up, so
`observed_burn_per_day` was a **lifetime average from install** and `runway_days` was computed
from it alone. That is the one shape a fuel gauge must not have. A table sits idle for months,
then fills with players — or gets pointed at by FINDING 26's free ingress flood — and the
lifetime average barely moves, because the months of quiet are still in it.

Measured, on a table left quiet for four hours and then given sustained load
(`the_gauge_reads_high_after_a_quiet_start`):

| | cycles/day |
|---|---|
| external truth over the window the gauge averages | 236,375,439,226 |
| the gauge's new one-hour window | 234,287,091,638 (0.9% low) |
| the gauge's **lifetime** average | 112,117,734,673 |

| runway on the same balance | days |
|---|---|
| what the canister reports now | **426** |
| what the load actually implies | 422 |
| **what a lifetime average gives — what shipped before this change** | **891** |

A **2.1x overstatement after four quiet hours**; on a real table with months of idle history
the lifetime average converges on the idle rate and the overstatement is unbounded. The source
comment beside `CYCLES_LATEST` already named this as the only failure mode that matters — *"the
gauge saying 'plenty of fuel' to a canister that is about to stop honouring withdrawals"* — and
the gauge was doing it.

**The fix.** A sliding one-hour window of tick samples (`CYCLES_RECENT`, bounded by age and by
count, summing only positive deltas so a top-up inside the window cannot cancel real burn), and
`runway_days` is now `liquid_balance` divided by the **larger** of the two rates. Reading low
costs an operator an unnecessary top-up; reading high costs every player at the table access to
their own money on a day nobody predicted.

> **A caveat on the first version of that gate, because it was wrong in a way worth recording.**
> The first test compared the gauge's one-hour average against an external measurement over a
> *different, shorter* window that happened to be mostly load-burst. It reported a 2.24x
> "overstatement" and would have failed any honest implementation: two averages over two
> different windows are not the same quantity, and a gauge that chased a 40-second burst
> instantly would be a worse gauge. The shipped test sustains the load for longer than the
> gauge's own window and measures the truth over exactly that window.

#### 4. A WARNING WHERE A PLAYER SEES IT — FIXED

`src/cleardeck_frontend/src/lib/cycleRunway.js` +
`src/cleardeck_frontend/src/lib/components/CycleRunwayNotice.svelte`, rendered in
`DepositModal.svelte` and `WithdrawModal.svelte` — the two screens money is committed from,
before any amount is typed. Warn below **60 days**, critical below **21**. Captured on the real
replica at `artifacts/screens/latest/deposit-desktop.png`:

> **This table has about 7 days of cycles left**
> Do not deposit. When a canister runs out of cycles it stops accepting every update call at
> once, including withdrawals, and money already inside it cannot be taken out until somebody
> tops it up.

with `5/5 protected notices on screen`, `0 occluded` — the notice is an ordinary in-flow block
with no `position`, no `z-index` and no `transform`, and `test-cycle-runway.mjs` asserts that
structurally so it can never grow the ability to cover the four protected notices.

Three states of it are not "low", and each is louder than silence:

* **UNREACHABLE.** [FINDING 24](SECURITY-FINDINGS.md#finding-24), now reproduced inside this
  tree: a frozen canister rejects **queries too**, so `get_cycle_status` goes dark at the exact
  moment its alarm is true. A failed read is therefore the LOUDEST state, not a caught error.
* **UNKNOWN.** `runway_days` is null for the first five minutes after every install, upgrade
  **and top-up**, because a top-up re-baselines the measurement. Null is never "fine".
* **UNSUPPORTED.** The running module has no such endpoint.

> **The false alarm this shipped with for one build, measured on the running system.** The two
> new fields were first declared as bare `nat`/`nat64`. Candid will not decode a record that is
> MISSING a non-optional field, so every reply from the **older module actually deployed on
> mainnet** became undecodable, the read threw, and the deposit screen of a table with **960
> days of runway** rendered *"This table is not answering"* — the alarm for a canister that has
> run out of cycles. The screenshot gate went green: it checks the protected notices and the
> money figures, not this banner. Both fields are `opt` now, and
> `test-cycle-runway.mjs` encodes an old-shaped record and decodes it against the shipped
> declarations, so the skew cannot come back silently.

#### 5. A READ-ONLY CI JOB THAT CANNOT DEPLOY — FIXED

`.github/workflows/cycles-monitor.yml` + `scripts/cycles-runway.sh`, twice a day. **The
workflow it replaced was itself a finding.** It ran `icp canister status`, which is
controller-only, so it base64-decoded `secrets.IC_DEPLOY_IDENTITY` into `/tmp` and made it the
default identity — every day, on a schedule, to read one number. A credential that can
`install_code` on a canister custodying real ICP ([FINDING 23](SECURITY-FINDINGS.md#finding-23))
was sitting in a scheduled job for the sake of a status line. It also alarmed on a flat **1 T**
cycles floor, which is 22 days on an idle table and **under seven hours** on one dealing
continuously, and it parsed a `.cycles` key it admitted in a comment it had never confirmed,
`continue`-ing silently when the parse produced nothing — so a wrong key name would have
reported *"all canisters above threshold"* forever.

The replacement authenticates as **nobody**: `get_cycle_status` is a public query, which is the
whole reason the canister publishes its own runway. `scripts/assert-read-only.sh` enforces it —
no mutating `icp` subcommand, no non-query canister call, no identity, no secret — and
self-tests against synthetic positives first, so a vacuous guard fails instead of passing.

Measured both ways on the local replica:

```
RED   table_1: 24 days left -- below the 60-day warning threshold
      table_2: 7 DAYS LEFT -- CRITICAL (1.269 T spendable, burning 0.160 T/day)
      table_3: 7 DAYS LEFT -- CRITICAL
  btc_table_1: 30 days left
      exit 1

(topped up with `icp canister top-up <id> --amount 50t -e local`)

GREEN table_1: 960 days (51.378 T spendable, burning 0.054 T/day)
      table_2: 100 days   table_3: 111 days   btc_table_1: 1139 days
      exit 0
```

**Two limits of it, stated rather than discovered later.**

1. **`lobby`, `history` and the asset `frontend` canister publish no runway.** They get a
   liveness probe only, which catches a canister that has ALREADY frozen and gives no warning
   beforehand. Closing that needs `get_cycle_status` on those canisters, in crates this change
   does not own.
2. **The mainnet balances are not readable from this machine.** The public dashboard API
   (`ic-api.internetcomputer.org/api/v3/canisters/<id>`) returns controllers, module hash and
   subnet and **no cycle balance** — checked, including `/cycle-balance`, `/metrics` and
   `/cycles`, all 404; the only network-wide cycle endpoints are aggregates. The only public
   per-canister read is the canister's own query, which is a mainnet call this wave is
   forbidden to make. So the runway of the live fleet is **UNMEASURED HERE** and the CI job is
   what will measure it. Note also that mainnet `table_1` reports module hash
   `9c0ed3a1…` against this tree's `cc47b16f…`/`713d5678…`, so the deployed module is a
   different build and may predate `get_cycle_status` entirely — the job reports that as a loud
   NO ANSWER rather than as a pass.

#### 6. THE TOP-UP: who pays, from where, and what happens if nobody does — DESIGNED, NOT BUILT

**The answer is an operator-funded reservoir, and it is emphatically not a levy on players.**

*Why not a levy.* Any mechanism that takes a fraction of a pot, a deposit or a withdrawal to
buy cycles **is a rake**. "No middleman, no house, 0% rake" is this product's headline claim,
it is one of the four protected notices, and it is gated on rendered pixels by
`tools/shots/test-rake.mjs`, which goes red on a rake of **1 e8**. A cycles levy would be a
rake with a good excuse, and a good excuse is what every rake has. **Nothing proposed here
touches player money.** If a future wave proposes something that does, that sentence is the one
to argue with.

*Can a table fund itself?* Only from money that is somebody's. A table's ICP is either escrow
(a player's) or a pot (a player's). There is no third pile. The canister cannot mint cycles and
cannot buy them without spending ICP it does not own. **So no: self-funding and the no-rake
property are mutually exclusive on this design, and that is a property of the product, not a
gap in the engineering.**

*The shape that works.*

| | |
|---|---|
| **who pays** | the operator, from operator funds, as a cost of running the service — the same party that pays for the deploy today |
| **from where** | a cycles reservoir: one canister holding cycles, funded from an operator wallet, whose only job is to hold and hand out cycles |
| **how it moves** | `deposit_cycles` on the management canister is **open to any principal and needs no controller rights**. TWO SEPARATE CLAIMS, WITH SEPARATE PROVENANCE, because conflating them is how a remedy nobody can run gets printed as if it worked. *Permissionlessness*: not measured in this wave — it is the property `src/no_peeking/dealer_canister/src/lib.rs` records as measured (*"open to all principals, no controller needed — measured"*) and that `src/guardian_canister/src/lib.rs` builds its recovery story on. *The command*: measured here, against the pinned CLI (icp-cli 1.0.2), as `icp canister top-up <id> --amount <n> -e ic`. `dfx` spells it `deposit-cycles`; **`icp` has no such subcommand**, and the first draft of the player-facing banner printed exactly that. `tools/shots/test-cycle-runway.mjs` now pins the spelling in the UI and in `scripts/cycles-runway.sh` together. **No new privileged actor on the table canister**, which matters because [FINDING 07](SECURITY-FINDINGS.md#finding-07) is open and [FINDING 23](SECURITY-FINDINGS.md#finding-23) is what a controller can already do. The reservoir needs no rights over a table and a table needs no method it does not already have. **Nothing here adds a way for a controller to change a player's balance** |
| **who pulls the trigger** | anybody. The CI job names the exact command in the incident it opens, and the player-facing banner names it too. A stranger who reads the alarm can fix it |
| **what happens if nobody does** | the table freezes. Every update is rejected; queries are rejected too; balances are intact and unreachable. A top-up at any later moment restores everything. **Only running to true zero destroys state**, and that is what the 60-day warning and the 21-day critical exist to keep far away |

*What is deliberately still missing, and why.* An automatic pull. A table that could pull
cycles from a reservoir on its own is a table with a standing claim on somebody's funds, and
sizing that claim safely under [FINDING 26](SECURITY-FINDINGS.md#finding-26) — where a free
ingress flood can burn 65x faster at zero cost to the attacker — is the hard half. An
attacker who can make the table spend can make the reservoir spend. Until the rate-limiting gap
that finding names is closed, a manual top-up against a two-month warning is the safer machine.

#### What remains OPEN

1. **A funded reservoir.** Money and a decision, not code.
2. **`get_cycle_status` on `lobby` and `history`**, so the monitor covers the fleet rather than
   the tables.
3. **A degraded mode.** Refusing new buy-ins while still honouring withdrawals is a better
   failure than serving both until everything stops at once, and nothing in the engine
   expresses it. It is now *possible* to express, because the canister knows its own runway.
4. **The rate-limiting gap in [FINDING 26](SECURITY-FINDINGS.md#finding-26)**, which is what any
   funding mechanism has to be sized against.

---

<a id="e-56"></a>
### E-56, three defects the on-chain clock work introduced, and how each was caught — STATUS: FIXED (wave 7)

**The middle one is high and would have voided real hands.** Recorded in full because this
repository's standing lesson is that every serious defect here was invisible to instruments
built by people who knew the answer, and all three of these were mine.

#### 1. The clock trapped on every single tick, and the watchdog is the only reason it mattered

First build of `on_clock_tick` opened with:

```rust
CLOCK_TICKS.with(|c| *c.borrow_mut() = c.borrow().saturating_add(1));
```

Rust evaluates the assigned value first and its `Ref` guard lives to the end of the statement,
so `borrow_mut` on the left hits an outstanding shared borrow. Every tick trapped:

```text
Panicked at 'RefCell already borrowed', src/table_canister/src/lib.rs:6365:29
[ic-cdk-timers] timer failed (code 5): IC0503 ... 'RefCell already borrowed'
```

repeated every 30 s for the whole run. **The clock kept firing anyway**, which is precisely the
property the repeating watchdog was chosen for: `ic-cdk-timers` reschedules an interval on
dispatch, before the callback runs. Had this been the self-rescheduling one-shot that the
finding's "obvious shape" note suggests, the first tick would have killed the clock silently and
the table would have been exactly as frozen as before — with a timer in the source, a dependency
in the manifest, and a code comment saying liveness was handled.

The statement is fixed; the comment above it keeps the wrong version, because it is the cheapest
possible reminder of what the watchdog is for.

#### 2. **high** — the clock VOIDED a playable hand, because it confused a stall with a stuck hand

`clock_should_abandon` asked whether the action clock had been expired for
`STUCK_HAND_GRACE_NS` (5 minutes). If so, `advance_table_clock` refunded every stake **before**
trying the ordinary timeout path. The reasoning was trap-safety: if the timeout path traps,
running the refund first is the only way it commits at all.

The money-safety fuzzer convicted it on its second seed, in the first run after the change:

```
CRITICAL: hand 7 was ABANDONED as unmovable at phase flop: no message could advance it.
4000000 e8s across 2 stakes returned to the players who put them in; nobody won the hand.
```

**Nothing was unmovable.** The fuzzer had jumped an hour of simulated time, so the clock was
five minutes past its expiry the *first* time anything looked at the hand. The ordinary path
would have folded the seat that did not act and played the hand out. Instead the hand was
voided and the pot handed back.

"Past its grace" is a claim about the WALL CLOCK. "Nothing can move this hand" is a claim
about ATTEMPTS. They are the same thing only while the clock is actually running, and they come
apart after any stall in which the canister does not execute:

* a subnet halt;
* **a canister frozen for want of cycles and later topped up** — which is [E-55](#e-55), so
  this defect and the one open custody risk compose: run out of cycles, get topped up, and
  every table with a hand in progress voids it on the way back up;
* a test harness advancing time in jumps, which is how it was caught.

**The fix.** The grace is now measured from **when the canister first saw the clock overdue**
(`CLOCK_STUCK_SINCE`), not from the timer's own `expires_at`. A stall buys no credit: the first
tick after it starts the grace and hands the hand to the ordinary timeout path.

Trap-safety is kept by a different mechanism, which is the part worth reading. The sighting is
written in `on_clock_tick`, a message that does nothing which can trap, and the *work* runs in a
message of its own (`set_timer(ZERO, …)`). A trapping timeout path therefore rolls back its own
message and leaves the sighting standing, so the grace keeps running and the refund still fires
in the end. The second message is only spent while a clock is actually overdue, so the
idle-table burn in the table above is unaffected.

**The gate, and why the fuzzer was not enough.** Reverting the fix so the grace is measured from
`expires_at` again leaves `cargo test --test fuzz` **GREEN** — the fuzzer catches the shape only
incidentally, through the unregistered `CRITICAL:` line, and a narrower regression walks past
it. So the property has its own gate now,
`tests/money_safety/tests/timers.rs::a_hand_stalled_past_its_grace_is_played_out_not_voided`,
which simulates the stall the only honest way: it advances an hour of time **with no ticks at
all**, i.e. a canister that does not execute, and then requires the hand to be *decided* — one
seat folded, somebody up by the pot, no `ABANDONED` line — rather than refunded. It is verified
red against the reverted fix.

#### 3. The cycles gauge read HIGH, which is the one direction that matters

`get_cycle_status` first sampled its origin balance in `start_clock` and compared it against the
balance read inside the query. It reported `observed_burn_per_day = 0` and
`measurement_is_meaningful = false` for an hour in which **1,843,363,200 cycles were
independently measured as burned** by the harness reading `pic.cycle_balance` from outside.

The cause is the same self-call that gives the clock its trap resistance: a canister executing
inside a timer callback has an outstanding prepayment for that call and its reserved response,
so the balance visible from inside a tick is depressed — by more than an hour of burn —
relative to the balance visible from a query. Subtracting one from the other measured the
prepayment, not the burn.

Both samples are now taken at the same point of the same kind of message, so the offset is
identical in both and cancels. The gate `idle_table_cycle_burn_and_runway` now asserts that
`get_cycle_status` reports a meaningful rate, and the value it reports (44,240,772,386
cycles/day) matches the externally measured 44,240,716,800 to five significant figures.

**A cycles gauge that reads high is the failure mode that matters**: it is the instrument
telling an operator "plenty of fuel" about a canister that is about to stop honouring
withdrawals. It would have shipped looking correct, because "no burn detected" on a
freshly-installed test canister looks exactly like a quiet canister.

---

<a id="e-57"></a>
### E-57, high, a player can walk away from a table with money in the pot and every surface tells them they have nothing — STATUS: FIXED (wave 7)

> **WAVE-7 COHERENCE NOTE, 2026-08-06.** The fix is real and I could not construct an invisible
> stake against it. But the two exit doors it added, `cash_out` and `leave_table`, were wired to
> `hand_is_stuck` — the raw wall-clock predicate — in the same wave that the timer work established
> that the raw predicate is wrong after a stall. That seam is [E-59](#e-59), and `cash_out` turned
> out to be the strongest door into it. The FINDING 18 fix was not reversed by the E-59 fix; the
> predicate underneath all three doors is what changed. **What E-59 moved in this entry's
> territory:** the exit doors now REFUSE during a stall instead of settling, and `cash_out`'s
> in-a-hand refusal carries the committed-stake sentence, so a leaving player is still never told
> they have nothing. Three of the four gates in `custody.rs` were rewritten to the new truth; the
> money assertions in all of them are unchanged.

**Status: executed, before and after, on modules built from this tree.**
Reproducers and gates: `tests/money_safety/tests/invariants/custody.rs` (four tests, in the
`invariants` target that `./scripts/dev.sh test` names), the M10 leg of
`tests/money_safety/src/invariants/custody.rs` evaluated on every fuzz step, and the
`stuck_hand_tests` block in `src/table_canister/src/lib.rs` for the mechanism at host speed.
Write-up: [SECURITY-FINDINGS.md FINDING 18](SECURITY-FINDINGS.md#finding-18).

**The measurement, on `306caef4…` — the module wave 6 shipped:**

```text
AUDITOR/before: phase=PreFlop pot=300000000 stuck=true bob_stake=298000000
    seat0 chips=296000000 bet=2000000    seat1 chips=0 bet=298000000
AUDITOR/cash_out(bob) -> Ok(0)   get_balance -> 0
    get_balance() -> 0 (escrow only)
    get_custody_status() is not answerable on this build: CanisterMethodNotFound
    get_table_view() carries no custody figure on this build

ORPHAN/after: cash_out alice -> 198000000, bob -> 199000000;
              phase=PreFlop pot=3000000 seats: (none)
```

`Ok(0)` while 2.98 ICP of bob's was in the pot; `get_balance` agreeing; and after both players
left, a **live pre-flop hand with a 3,000,000 e8 pot and zero seated players**, which nothing
was ever going to move.

**The same sequence on the fixed module:**

```text
CRITICAL: hand 1 was ABANDONED as unmovable at phase preflop BY AN EXIT DOOR ...
          300000000 e8s across 2 stakes returned to the players who put them in,
          including the leaver's
AUDITOR/cash_out(bob) -> Ok(298000000)   get_balance -> 298000000
AUDITOR/withdraw(298000000) -> wallet 999701980000 -> 999999970000
AUDITOR/after: phase=HandComplete pot=0
ORPHAN/after: phase=HandComplete pot=0 seats: (none)
```

#### What was changed, and why it is not the change the auditor asked for

The auditor asked for three strings: have `cash_out`/`leave_table` report the amount still
committed, surface it on `get_balance` or the table view, and name `abandon_stuck_hand` in
`withdraw`'s refusal. Two of those three are here. The first one is not, and the reason is worth
stating because it looks like a shortfall:

* **`Ok = 0` could not be made to say `Ok = 0, 298_000_000 still in a stuck pot`.** The reply is
  `variant { Ok : nat64; Err : text }` and a `nat64` carries no sentence. Widening it to a record
  breaks every checked-in client IDL (`src/declarations/**`, which `+page.svelte` decodes
  `Number(result.Ok)` from) — files this pass does not own, in a repo where four agents are
  editing concurrently.
* **So the state was removed instead of annotated.** `cash_out` and `leave_table` now SETTLE a
  hand that no message can move, before they vacate the seat. The stake comes back into the
  stack and leaves with its owner, and `Ok(298_000_000)` is the true and complete number. That
  is strictly stronger than the annotation: an annotated `Ok = 0` still leaves 2.98 ICP in a pot
  on a table the player has left.
* It grants **no new power**. `abandon_stuck_hand` is already callable by any principal on a
  stuck hand, so the caller could reach the identical state in one extra call.
* And **the last player out turns the lights off**: after a departure that empties the table, a
  live hand is settled the same way. Nobody holds cards, so nobody can win it.

#### The one thing that could have re-created FINDING 15, and the guard against it

`settle_unmovable_hand` runs INSIDE `cash_out` and `leave_table`. `apply_payouts` TRAPS on a
plan that does not conserve, and a trap rolls the whole message back — so a bad plan would have
taken the player's exit with it and shut the door they were walking through. That is
[FINDING 15](SECURITY-FINDINGS.md#finding-15) re-created by the fix for
[FINDING 18](SECURITY-FINDINGS.md#finding-18). The plan is therefore checked first and a
non-conserving one is declined and logged rather than acted on; the player still leaves.

`the_refund_plan_conserves_for_every_state_the_exit_doors_can_meet` records honestly that the
guard could not be reached: `refund_every_stake` derives both sides of `conserves()` from one
list of stakes, so no state the test could build made it false. It asserts the reason instead of
pretending to have reached the branch.

#### What is still visible, and why the surfaces are not redundant

One case survives the strong fix by design: a player folds out of a hand that is **still
moving** — an ordinary, contested departure that correctly leaves their stake in the pot — and
the table stops moving afterwards, with them already gone. For that case:

* `get_custody_status()` (new, caller-scoped query) reports `committed_in_pot`,
  `committed_is_stuck`, `abandonable_in_ns`, a `total`, and an `advice` string that NAMES
  `abandon_stuck_hand`;
* `TableView` carries `my_committed_in_pot` and `hand_is_unmovable`, so the call every client
  already makes on a loop cannot fail to show it;
* `withdraw`'s refusals — both "Insufficient balance" and "Cannot withdraw while in a hand" —
  carry the same sentence, from the same function, with the exact e8 figure.

Measured:

```text
DEPARTED/withdraw refusal: Insufficient balance. Have: 19.9800 ICP, requested: 20.0000 ICP.
  0.0200 ICP (2000000 e8s) of yours is still committed to hand 1, and that hand can no longer
  be moved by any message, so nobody can win it. Call abandon_stuck_hand() -- any principal
  may -- and every stake goes back to whoever put it in, including yours, into your
  withdrawable balance.
```

#### The UI

`WithdrawModal.svelte` reads `get_custody_status()` on open and renders the committed figure
under the balance it corrects, with a **Recover … now** button when the hand is unmovable.
`PokerTable.svelte` renders the same figure in the wallet panel. Both are in the document flow
with no positioning and no z-index, so neither can cover the four protected notices (HARD
RULE 2, and E-52 is what happens otherwise).

One line in `PokerTable.svelte` mattered more than the banner: the Withdraw button was
`disabled={tableBalance <= 0}`, which locked the one player who most needed that screen out of
it — the auditor, whose escrow read zero **because** their 2.98 ICP was in a pot. It now opens
whenever a stake is outstanding.

#### The gate, and what it does not prove

**M10 CUSTODY VISIBILITY**, `tests/money_safety/src/invariants/custody.rs`:

> If a principal has a positive stake in a pot, at least one surface **that principal can read**
> must say so.

Every other reader in that harness is a controller, which is exactly how this stayed invisible:
`get_table_state` is controller-only, M1 balanced, and M9's drain reported `fully_drained`
because the money genuinely was reachable — by a method nothing named. M10 reads the canister as
the player, and treats a method that does not exist, a rejected call and an undecodable reply
all as "this surface reports nothing", which is why it convicts the pre-fix module cleanly
instead of failing to compile against it.

**What it does not prove.** On this tree the on-chain clock added by [E-54](#e-54) also refunds a
stuck hand, within 30 s, so a replica fixture that simply advances time can be satisfied by the
clock rather than by the exit door. The custody tests therefore advance time WITHOUT executing a
round and then assert on the canister's own log line — `BY AN EXIT DOOR` — so the gate fails if
the exit-door code is deleted even while the clock is still there.

---

<a id="e-59"></a>
### E-59, high, THE FOURTH CROSS-AGENT DEFECT: two clocks, one hand, two sets of recipients — STATUS: FIXED (wave 7)

> **Status:** **FIXED 2026-08-06**, reproduced first. Found by the wave-7 coherence pass on module
> `cb26fb9495fbe2084590ff087245968f33bb78460b3c2c977d06c002e632c105`. Full write-up and every
> measurement: [FINDING 25](SECURITY-FINDINGS.md#finding-25), which is raised from medium to high
> by this entry. This register entry is the coherence story; that one is the evidence.
>
> **Where it was:** `hand_is_stuck` against `clock_should_abandon`, and the five surfaces that read
> the first one: `abandon_stuck_hand`, `cash_out`, `leave_table`, `get_custody_status`,
> `get_stuck_hand_status`, `TableView.hand_is_unmovable`.
>
> **Reproduced, then fixed, then gated.** The reproduction and the gate are the same file:
> `tests/money_safety/tests/stall_agreement.rs` (**M13 ONE BELIEF**), which forks **138 states**
> and drives each one down two arms.
>
> | | before (module `bf2bb17…`) | after |
> |---|---|---|
> | states compared | 138 | 138 |
> | rows whose two arms paid different recipients | **118** | **0** |
> | `Err` replies that changed the table | **20** | **0** |
> | doors that voided a hand the clock plays out | not separately counted (see note) | **0** |
>
> **Read the 118 with its caveat.** The first run used a blunter criterion -- exact recipient
> equality on *every* row, including `leave_table` by a seated player, whose mid-hand use is an
> ordinary fold and legitimately changes the outcome. At most 30 of the 138 rows are that
> confound, so at least 88 were the defect. The gate as it ships is narrower: exact equality for
> the doors that are not ordinary game actions, plus the log-based "no door voided a hand the
> clock plays out" on **all** rows. The after-run is zero on every count under both criteria.
>
> ```text
> cd tests/money_safety && cargo test --test stall_agreement -- --nocapture --test-threads=1
> ```

**Why this is a coherence defect and not a bug in anybody's region.** The timers agent found the
right rule and wrote it down in `advance_table_clock`:

> *"Past its grace" is a statement about WALL CLOCK. "Nothing can move this hand" is a statement
> about ATTEMPTS. They are only the same while the clock is actually running.*

They applied it to the predicate the timer uses. In the same wave, the custody-visibility agent
wired three new doors onto the predicate that still reads raw wall clock — because that is the
predicate that existed, and it is the one every document describes. **Neither was wrong inside
their own region.** Reviewing either diff alone shows nothing.

**Measured, one state, two doors:**

```text
=== ARM: the clock alone, ZERO ingress messages
  pot=4000000  phase=PreFlop  on_clock=alice  stakes=[2000000, 2000000]
  clock resolved it after 2 rounds
  RESULT phase=HandComplete   alice=+0        bob=+4000000    sum=4000000

=== ARM: abandon_stuck_hand, called by the seat on the clock
  abandon_stuck_hand(alice) -> Ok(4000000)
  RESULT phase=HandComplete   alice=+2000000  bob=+2000000    sum=4000000
```

Correct totals, wrong recipients, every conservation invariant silent. That is the same signature
as E-36/FINDING 17 (wave 6), FINDING 13 (wave 3) and E-05 (wave 2).

**Why no gate in this project could reach it, and what the gate had to be.** `World::advance()` is
`pic.advance_time(d); pic.tick()` — **it always lets a round run**, so the on-chain clock always
wins the race and the window never opens. `World::advance_time_only` is the only thing that opens
it; it appeared in exactly three places in the tree, all in
`tests/money_safety/tests/invariants/custody.rs`, and **all three used it for QUERIES only.** No
test in this repository had ever sent an UPDATE through the window. The fuzzer jumps hours of
simulated time and cannot reach it for the same reason, which is why `make fuzz-default` is green.

#### THE FIX: one predicate, and it is about ATTEMPTS

`clock_should_abandon` is **deleted**. `hand_is_stuck` is now the only predicate, every one of the
six surfaces reads it, and it is a statement about what this canister has WATCHED fail:

> a hand is stuck when this canister, **while executing**, has handed it to the ordinary
> resolution path in at least `STUCK_HAND_MIN_OPPORTUNITIES` (3) separate committed messages
> spanning at least `STUCK_HAND_GRACE_NS` (300 s), and the hand has not moved.

The evidence lives in `STALL_WITNESS`, written by `note_stall_opportunity` from exactly two places
and nowhere else:

* `on_clock_tick`, in a message that does nothing that can trap, before the attempt is dispatched
  in a message of its own. That is what makes a resolution path which **traps** still count as an
  opportunity given, which is the case the escape hatch exists for;
* `check_timeouts`, immediately after `advance_table_clock` returns. `check_timeouts` is
  permissionless, so **a canister whose timer never re-armed still has a route to abandonment that
  any player can drive** — three of those calls across a grace period. That is what keeps
  [FINDING 15](SECURITY-FINDINGS.md#finding-15)'s fund lock from being rebuilt by coupling the
  manual door to timer state, which is the objection the original write-up raised against this fix.

The witness is keyed on `(hand_number, action_timer.expires_at)`, so **it clears itself the moment
the hand moves** — a new hand, a player acting (which replaces the timer), the time bank extending
it, the hand ending. Nothing has to remember to reset it. It is **not persisted across upgrades**,
which is correct rather than lazy: an upgrade is exactly a period in which the canister was not
executing, so its evidence does not survive one.

Two consequences worth stating plainly:

* an hour-long stall now buys **no** credit toward abandonment on **any** door;
* the on-chain clock may now act on the live-hand-with-no-clock arm, which it was previously
  forbidden to do. That is not a widening of the automatic door, it is the same evidence standard
  applied to both arms: that state must also be watched for a full grace before anything acts on it,
  where it used to be abandonable the instant anybody looked.

#### THE OTHER HALF: an `Err` that commits

The settle in `cash_out`/`leave_table` ran *before* the seat lookup, and returning `Err` from an
ic-cdk update is a normal reply rather than a rollback, so
`mallory (never at the table) cash_out -> Err("Not at table")` settled the hand
(`phase PreFlop -> HandComplete, pot 3000000 -> 0`). Measured on **20 of 138** states.

Both functions are restructured so that **every refusal is decided before the first mutation**: the
seat lookup and the in-a-hand guard come first, and the code says so with a
`---- FROM HERE ON NOTHING RETURNS Err ----` line. `cash_out`'s in-a-hand refusal now also carries
the committed-stake sentence, so [FINDING 18](SECURITY-FINDINGS.md#finding-18)'s guarantee — a
player is never told they have nothing while the canister holds their money — is carried by the
refusal instead of by voiding a playable hand.

#### What the harness gained

`World::advance_time_only` is unchanged in behaviour and rewritten in doc: it now says, at the top,
that the point is to send an **update** through the window. **M13 ONE BELIEF**
(`tests/money_safety/tests/stall_agreement.rs`) is the gate. Per scenario it forks one control
row plus three doors × (every seat + one principal who has never sat down); over 12 scenarios that
is **138 states**, and on each one it asserts

1. the door did not void a hand the clock plays out — checked on the canister's own
   `ABANDONED as unmovable` log line, so it holds for `leave_table` too, whose mid-hand use is an
   ordinary fold and legitimately changes the outcome;
2. for the doors that are not ordinary game actions, every principal ends with the same e8s in both
   arms — **not the same total, the same amount each**;
3. no `Err` reply changed the table.

A second test in the same file, `m13b_no_surface_calls_a_playable_hand_dead`, is the cheap half:
one world, no arms, QUERIES only. In the stall window it reads `get_stuck_hand_status`,
`get_custody_status` and `TableView.hand_is_unmovable` **as each seated player** and fails if any
of them calls the hand dead — including if the advice string still contains *"no longer be moved"*
or *"nobody can win it"*. That is the sentence the losing player was shown, and a query executes no
round, so nothing that reaches time through `World::advance` could ever have caught it.

Three tests in `tests/money_safety/tests/invariants/custody.rs` pinned the old belief and are
rewritten rather than deleted: they now assert that the exit doors REFUSE during a stall and that
the clock then plays the hand out. One of them,
`m10_a_departed_stake_in_a_pot_that_stopped_moving_is_visible_to_its_owner`, used to assert that a
player who had **folded by leaving** could call `abandon_stuck_hand` and get her stake back. That
assertion was E-59 in miniature, inside the gate for FINDING 18.

Three unit tests in `lib.rs` pinned it too, including one called
`the_clock_refuses_the_no_clock_branch_that_abandon_stuck_hand_accepts`, whose whole subject was the
divergence. They are replaced with tests that pin the single predicate, and the replacements say in
their doc comments what they replaced and why (HARD RULE 7).

> **⚠ THE GATE IS NOT WIRED INTO A TARGET YET.** `scripts/dev.sh cmd_test` names every money-safety
> binary explicitly — that is deliberate, and it is the fix for [H-17](#h-17) — so a new binary is
> run by nothing until it is named. `stall_agreement` is a new binary. It needs exactly one line,
> next to the `--test timers` line in `scripts/dev.sh`, in the file this change did not own:
>
> ```sh
> cargo test --test stall_agreement -- --test-threads=1 &&
> ```
>
> This is [H-38](#h-38) and [H-17](#h-17) all over again, recorded rather than done, because a
> gate that runs in one agent's terminal and in no target is the shape that has already cost this
> project a wave. **Budget ~740 s** for the full binary; `m13b_no_surface_calls_a_playable_hand_dead`
> alone is under 3 s and is the half that gates the advice string.

<a id="e-60"></a>
### E-60, medium, `make hygiene` is RED: the screenshot manifest is a tracked 7.6 MB file — STATUS: FIXED (wave 7)

> **Status:** FIXED. `manifest.json` is gitignored build output; the tracked evidence is
> `INDEX.md` plus a new `verdicts.json` (**7,612,380 → 10,767 bytes**), and `make hygiene` now
> fails on any tracked file over 512 KiB under `artifacts/`, so the 10x-a-wave growth cannot come
> back silently. **Where the evidence lives now is spelled out at the end of this entry.**
>
> Measured before the fix, 2026-08-06: `./scripts/dev.sh hygiene` exited **1**.

```text
==> no large or binary files added
    48 untracked path(s), 17023 KiB total
    ✓ no binary or image files would be committed
    ! untracked payload exceeds 4 MiB; check for a stray artifact directory
==> result
    ! repo hygiene FAILED
```

`artifacts/screens/latest/manifest.json` is **tracked** and went from `763,703` bytes at `HEAD` to
`7,612,380` — 4.48 MB of it in `scenes`, from the per-figure pixel/occlusion sampling the wave
added. `artifacts/screens/4af6dc6/manifest.json` is an untracked second copy of the same 7.4 MB.
The PNGs are gitignored; the manifests are not.

**The notice half of hygiene is GREEN** — all nine protected-notice checks pass and nothing has been
weakened since `ceacc37` — so this failure is purely payload, and that is exactly why it is
dangerous: `make hygiene` is one of the gates that guards the four protected notices, and it now
exits 1 for a reason nobody needs to read. Wave 6's REG-09 lesson, one gate over.

#### What was decided, and WHERE THE EVIDENCE LIVES NOW

A run produces three things and only two of them are evidence:

| artifact | tracked? | what it is |
|---|---|---|
| the PNGs | **no**, since wave 1 | the pictures. ~10 MiB a run, regenerated by every sweep |
| `manifest.json` | **no**, as of this fix | every figure, every pixel sample, every occluder pair, every classified token. 7.6 MB and growing 10x a wave. **No human has ever read it.** It stays on disk next to the PNGs it describes, for whoever ran the sweep |
| `INDEX.md` | yes | the human-readable table: every scene, its verdict, its assertions, its failure text, the file each shot went to. 25 KB |
| `verdicts.json` | yes, **NEW** | the machine-readable spine: one row per (scene, viewport), verified or not, and the headline of every failure. **10,767 bytes** for the run that produced a 7,612,380-byte manifest |

Nothing about a run's verdict is lost — `verdicts.json` carries all fourteen reds of the
`4af6dc6` sweep with their messages, and `tools/shots/backfill-verdicts.mjs` regenerated one for
every run directory already on disk before the manifests left the index. What is gone from git is
the working-out.

**And it is now load-bearing rather than decorative.** `verdicts.json` is what
`./scripts/dev.sh shots-verdict` gates on (see [E-63](#e-63)), so the small tracked file is the
one a wave actually has to answer to.

**Two teeth so this cannot recur:**

* `make hygiene` fails on any **tracked** file over 512 KiB under `artifacts/` — the previous
  check only measured UNTRACKED payload, which is why committing the 7.6 MB file made hygiene go
  green while making the problem permanent; and
* the untracked-payload counter no longer bills a staged **deletion** for the bytes of a file that
  is still on disk, which would otherwise have made removing these very files look like adding
  23 MB.

**Fix:** either gitignore the manifests alongside the PNGs and keep `INDEX.md` as the committed
evidence, or split the per-figure pixel samples into a separate, ignored sidecar and leave the
verdicts in the manifest.

<a id="e-61"></a>
### E-61, high, the no-rake gate reads a field that does not exist on the record it reads — STATUS: FIXED (wave 7)

> **CONFIRMED ON A LIVE SWEEP.** No `RAKE TAKEN` line on any of the 24 wave-8 shots; the
> archived-hand read is correct against real canister output and the acknowledgement was deleted.
> This does **not** answer the wave-8 critic's separate objection that both clauses of the gate
> are tautologies on canister-produced data ([FINDING 32](SECURITY-FINDINGS.md#finding-32),
> still OPEN): a gate that no longer cries wolf is not the same as a gate that can convict.

> **Status:** FIXED. The fold is now a pure exported function, `foldArchivedHand`, that reads
> `rake` off `get_hand`'s `HandHistoryRecord` and treats a missing field as a structural failure;
> `tools/shots/test-rake.mjs` proves it goes red on a rake of **1 e8** and green on an honest hand,
> with no replica.
>
> **The objection the previous wave raised was right and is answered rather than ignored:**
> changing a gate you cannot re-run is how the next wave inherits a false green. So the fix came
> with a way to run it. 22 cases, all offline: the first seven read the field names straight off
> the live Candid declarations (proving `HandSummary` really has no `rake` and
> `HandHistoryRecord` really does), and reverting the one line reproduces `rake=NaN` exactly and
> fails 10 of the 22.
>
> Root-caused 2026-08-06 from the last full sweep's own manifest (`artifacts/screens/latest/`, run
> at `4af6dc6`).

`handhistory` is UNVERIFIED at both viewports on two failures that are the same missing field:

```text
RAKE TAKEN: hand 1 recorded rake=NaN e8s. ClearDeck publishes a no-rake property;
a non-zero rake contradicts it.
hand 1: history says total_pot=2400000000 but the winners were paid 2400000000 with rake NaN
        (difference NaN e8s)
```

`tools/shots/lib/chain-agreement.mjs:1546` builds its record from
`history.get_hands_by_table(...)`, which returns **`vec HandSummary`**
(`src/declarations/history/history.did:113-115`). `HandSummary` carries `total_pot` but **no
`rake`** (`:41-50`); only `HandHistoryRecord`, returned by `get_hand(hand_id)`, does. So
`Number(r.rake)` is `NaN`, `NaN !== 0` is true, and `totalPot !== awarded + NaN` is true.

**The comment thirty lines above it, at `:1514`, reads:**

> *"An earlier version of this function read `rec.total_pot` off the TABLE record. That field does
> not exist, so `Number(undefined)` was NaN and every comparison was vacuous — reported as a
> disagreement only by luck. **Absence of a field is now a structural failure, never a silent
> NaN.**"*

The author found this exact mistake, fixed it in one field, wrote down the rule, and left the
identical mistake in the adjacent line. That is the standing lesson, executed on itself.

**Why it is HIGH and not an annoyance.** No-rake is one of the four properties that may never be
weakened, and this is the only gate that asserts it against the permanent archive. It reports a
rake being taken on **every archived hand**, forever, so the one alarm that guards the property is
guaranteed to be ignored.

**Fix (one line, plus one guard):** read the full record with `get_hand(hand_id)`, which carries
`rake`; and honour the rule the file already states by making a missing field a structural failure
rather than letting it become `NaN`.

#### What the fix found on the way in

The blind spot the standing lesson predicts was one level up from the bug, in the same function.
`byHandNumber` was populated inside a `try`, and the `catch` wrote the error into
`byHandNumber.set('error', ...)` — **a key nothing ever read**. The same for a run with no history
canister id. In either case every `hist` came back `undefined`, every assertion in the row loop was
skipped, and the scene could go **GREEN with the no-rake property asserted by nothing at all**. Not
a wrong answer: an absent one, which is the shape that keeps surviving here. Both branches are now
structural failures, and a hand the table has recorded that the archive does not have is one too.

The fold also cross-checks the archive's two read paths against each other, because a
`get_hands_by_table` that says one pot and a `get_hand` that says another means the modal's number
depends on which call the client happened to make.

<a id="e-62"></a>
### E-62, high, the ICP deposit floor was below the withdrawal floor, so money could arrive at the advertised minimum and never leave — STATUS: FIXED (wave 7)

> **Status:** FIXED. The DEFECTS twin of
> [FINDING 27](SECURITY-FINDINGS.md#finding-27), which carries the evidence.

`deposit()` accepted 20,000 e8s of ICP; `withdraw()` refused anything below `100_000`. Every
balance in **[20,000, 100,000)** was money the canister had taken and would not give back —
`withdraw` refused it as below the minimum, `buy_in` refused it as four orders of magnitude below
a buy-in, and `get_custody_status` reported it as an ordinary healthy balance with `advice = ""`.
The third auditor deposited exactly the amount the product advertises and it was stuck.

**Fixed as an invariant, not as a number.** `src/table_canister/src/lib.rs` now states the rule
and has the compiler enforce it:

```rust
const _: () = assert!(ICP_MIN_WITHDRAWAL_AMOUNT <= ICP_MIN_DEPOSIT_AMOUNT, "FLOOR INVARIANT BROKEN: ...");
const _: () = assert!(ICP_MIN_WITHDRAWAL_AMOUNT >  ICP_TRANSFER_FEE,       "FLOOR INVARIANT BROKEN: ...");
```

Putting the old `100_000` back does not produce a failing test, it produces a **build failure**
naming FINDING 27. The deposit floor is a named constant per currency for the first time
(`ICP_MIN_DEPOSIT_AMOUNT` / `BTC_MIN_DEPOSIT_AMOUNT`) with a `Currency::min_deposit()` beside the
existing `min_withdrawal()`; that it was an anonymous literal inside `deposit()` is *why* nothing
in the tree could compare the two.

**And the half that equal floors do not fix.** Escrow balances are not only made of deposits: an
odd-chip split, a partial withdrawal or a `claim_external_deposit` sweep that paid the ledger fee
out of the amount all leave balances under any floor, and each is money the canister holds. So
`withdraw` waives the floor for the one request that cannot be a mistake — the caller's WHOLE
remaining balance, at any size the ledger can move — and the refusal below that names the network
fee rather than a policy number. Six PocketIC tests in
`tests/money_safety/tests/deposit_floor.rs` walk the old dead band, the sub-floor residue and the
boundary at the fee itself; `ui_limits.rs` reads the relation off the source and both modals
mirror one number.

<a id="e-63"></a>
### E-63, high, a money figure was painted over by the felt on a phone, the pixel gate said so, and nobody acted for a whole wave — STATUS: FIXED (wave 8)

> **CONFIRMED ON PIXELS.** The wave-8 sweep reached `table-allin/mobile`, the exact shot that
> carried this: *"pixel gate: 25 money/equity/card figures on screen, 25 with something
> overlapping them, 81 pairs pixel-tested, **0 occluded**"*. The acknowledgement was deleted from
> `artifacts/screens/acknowledged-reds.json`. That shot is still red for a different reason
> ([E-76](#e-76), the felt floor), which is why the entry moved rather than vanished.

> **Status:** FIXED in the stylesheet and measured against the component's own stylesheet by
> `tools/shots/test-dock-overflow.mjs`. **Not** re-measured by the screenshot sweep, which needs a
> local replica this session cannot start. Acknowledged in
> `artifacts/screens/acknowledged-reds.json` until a sweep proves it.

At 390x844, on `table-allin`, the run at `4af6dc6` reported:

```text
OCCLUSION FAILED: 1 of 25 figures on screen are covered —
8.7% of "0.20 ICP" (span.committed-value) covered by div.stage
```

The recorded geometry says exactly what happened. The figure's box was `y=731 h=21`; `div.stage`
was `y=132.59 h=605`, so the stage's bottom edge lay over the figure's top 6.6 px, and the
hit-test at the centre of the figure returned `span.equity-badge` — a seat pod hanging off the
felt. `.action-dock { height: var(--dock-h) }` is a FIXED height and the wallet panel is
**87.4 px** against a **36 px** row on a phone whenever a committed stake is on show. A too-tall,
UNPOSITIONED child does not clip the page; it spills out of the box, and `.stage` is
`position: relative`, so the stage paints after it (`paintsAfterTarget: true`, paint index 45
against 40).

**The fix is not a z-index.** Raising the dock above the stage would let dock content cover the
felt, and *"NO RAKE"* is printed on the felt (HARD RULE 2). The dock is now sized by its contents
(`min-height` instead of `height`), so the panel stays inside its own box and paint order stops
mattering.

**What it cost, and the trade this nearly hid.** The dock's room comes out of the stage, and the
stage is what sizes the felt. The first version of this fix let the block take all 51 px it wanted
and dropped the felt from **50.7% to 44.1%** of the frame, against `felt-area.mjs`'s **45%** mobile
floor — an occlusion green bought with a felt red, one gate paid for with another. The fixture
caught it. Three declarations now take 14 px back out of padding, gap and a margin that does
nothing in a row; the block needs 37 px and the felt lands at **46.4%**, and the fixture fails if
it ever creeps back toward the floor. Nothing is hidden: the label, the money figure and the whole
sentence all still render.

**Why the red was walked past is the more important half, and it is not carelessness.** The sweep
exited 1 and wrote the failure into its manifest. But the only thing that can see a rendered-pixel
failure is a sweep, a sweep needs a live replica, and the replica has been unstartable for two
waves — so no gate anybody could run was red. That is fixed by
[`./scripts/dev.sh shots-verdict`](../tools/shots/verdict-gate.mjs), which `make hygiene` runs:
the LAST RECORDED VERDICT is itself a gate, it needs no replica, and a red with no entry in
`artifacts/screens/acknowledged-reds.json` naming a defect in this file fails it. An entry that is
no longer red fails it too, so the ledger cannot rot into a list of permanent excuses.

<a id="e-64"></a>
### E-64, medium, the committed-stake readout renders real money that no gate tied to any canister figure — STATUS: FIXED (wave 8)

> **CONFIRMED ON PIXELS.** The wave-8 sweep reports *"0 unaccounted for"* in the token census on
> every table scene, both viewports — `table-preflop`, `table-facing-bet`, `table-allin`,
> `table-sidepots`. All four acknowledgements deleted.

> **Status:** FIXED. Same measurement caveat and same acknowledgement as [E-63](#e-63).

Five scenes at `4af6dc6` were red on:

```text
TOKEN CENSUS FAILED: 2 of N numeric tokens on screen are asserted by nothing —
  "0.20" in ... span.committed-value / "1" in ... span.committed-note
```

The census was right. The custody-visibility work added the FINDING 18 readout — *"In the pot /
0.20 ICP / Yours, in hand 1, until the hand settles"* — and nothing scraped it, so a real ICP
amount was on screen with no canister figure behind it. This is the surface that exists because a
player was once told they had nothing while their money sat in a pot; it being unasserted is
exactly the case where a wrong number would matter most.

**Fixed with a chain check, not an allowlist entry** — allowlisting a money-shaped token is the one
thing the census exists to refuse. `dom-scrape.mjs` reads `.committed-value` and
`.committed-note`; `chain-agreement.mjs` asserts the amount against
`get_table_view().my_committed_in_pot`, the hand number in the sentence against
`hand_number`, and the panel's red "stuck" state against `hand_is_unmovable`. Two `CHAIN_SITES`
entries connect the tokens to those checks.

**And the silence is checked as hard as the value.** If the canister says the caller has money in
the pot and no `.committed-value` is on screen, that is now a structural failure. Otherwise
FINDING 18 coming back would look identical to "this scene has no committed block", which is what
an absent assertion always looks like.

<a id="e-65"></a>
### E-65, medium, every lobby row's NAME quotes blinds the canister does not have — STATUS: OPEN

> **Status:** OPEN. Recorded here so it can be acknowledged rather than skimmed past; not this
> wave's work. Red on 4 of the 24 recorded shots (`lobby` and `toast-notices`, both viewports).

```text
LOBBY CHAIN DISAGREEMENT: lobby row NAME "6-Max - 0.01/0.02" quotes a small blind:
  screen "0.01" means 500000..1500000 e8s, canister says 5000000 e8s (screen is 0.200x the chain)
lobby row NAME "9-Max - 0.01/0.02" quotes a big blind:
  screen "0.02" means 1500000..2500000 e8s, canister says 20000000 e8s (screen is 0.100x the chain)
```

The blinds are written into the table's registered NAME in the lobby canister and the config says
something else, so the lobby advertises stakes 5x and 10x below what a player will actually be
charged when they sit down. The same two figures are repeated in the join nudge, which is the
other half of the census failure on those scenes. It is the same class as
[T-26](#t-26) — a number written twice is a number that will disagree with itself — one canister
over: the fix is for the row to render `config.small_blind`/`config.big_blind` rather than to
parse a name, and for the name to stop carrying figures at all.

<a id="e-66"></a>
### E-66, high, the permanent hand record named the wrong people, and the shuffle specification told verifiers to trust it — STATUS: FIXED (wave 8)

The archive half is [SECURITY-FINDINGS.md FINDING 30](SECURITY-FINDINGS.md#finding-30), which
carries the full reproduction. This entry exists for the half that is a DOCUMENT defect, because
it would otherwise be filed as a code fix and the document would stay wrong.

`docs/SHUFFLE-SPEC.md` section 4 said:

> *"Count it from the hand's own record — every seat the history shows with cards, plus any that
> folded."*

The list it told a verifier to count was built from the seats **as they stood when the hand
settled**. On any hand somebody left it was short by one; on any hand somebody joined it was long
by one. Section 4 offsets the board by that number, so following the specification produced a
different board from the right seed — and the specification's own words say that if the document
and the code disagree, *"that is a bug in ClearDeck, not in your verifier"*.

Measured on the archived hand in FINDING 30, seed
`96edd81596bc467971d030ca90cb36fd6fe8e027107679271a6e23c46ac794ee`, by both outsider verifiers:

```text
--players 4  ->  flop 9d 4d 8c   turn 2h   river Js     # dealt, and in the record
--players 3  ->  flop 6h 5h 9d   turn 8c   river 2h     # what section 4 told you to compute
```

**Fixed on both sides.** The record now carries `dealt_in`, the deal's own ordered list of the
seats it fed, written by the deal loop as it deals; section 4 says to READ it, and says that a
`null` `dealt_in` means the hand's board cannot be reproduced and must not be guessed at. The
archive's `check_recorded_hand` prints the verifier command with that hand's `P` filled in, and
prints an explanation instead of a command when the record does not state `P`.

The gate is `the_board_can_be_reproduced_from_the_archived_record_alone` in
`tests/money_safety/tests/invariants/archive.rs`, which reproduces the board from the archived
record alone and asserts that the OLD `P` gives a *different* board — so it cannot pass vacuously
on a hand where the two agree.

<a id="e-67"></a>
### E-67, medium, the archive's generated JS bindings are stale, so the fairness fields do not reach the UI — STATUS: OPEN

`src/declarations/history/history.did` and `src/history_canister/history_canister.did` were
regenerated with `candid-extractor` for the FINDING 30 fix and now carry `dealt_in`,
`contributed`, `left_mid_hand` and `dealt_in_count`.
`src/declarations/history/history.did.js` — the `idlFactory` the frontend
(`src/cleardeck_frontend/src/lib/canisters.js`) and `tools/shots` actually decode with — was not
regenerated and does not declare them.

Nothing breaks: Candid record subtyping means the JS decoder silently drops fields it does not
declare. That is the problem. **The hand-history view cannot show a player how many people were
dealt into their hand, which is the number they need to check it**, and it will keep not showing
it with no error anywhere.

There is a consumer waiting for exactly this field.
`src/cleardeck_frontend/src/lib/components/HandHistory.svelte`, in `seatCardChecks`:

> *"The hand log does not record which seats were dealt in, so this assumes the seats it saw, in
> ascending order, are that list. If the assumption is wrong the codes simply will not match and no
> tick is shown: a wrong guess can never produce a false confirmation."*

That was the right way to handle not knowing — it fails closed, so it never lies — and the premise
is now false: the hand log DOES record it, on both canisters. Every hand somebody left currently
shows a player no verification tick when it could show them a correct one. The same applies to the
table canister's own declarations, which are separately stale
(SECURITY-FINDINGS.md, "FINDING 03 -- the committed `table_canister.did` does not describe the deployed code") and were deliberately NOT regenerated in this pass:
four agents were mid-edit in `src/table_canister/src/lib.rs`, and extracting its Candid would have
committed their unfinished interfaces.

Two things worth noting for whoever picks this up:

* the same file was ALREADY stale before this change — it was generated before
  `check_recorded_hand` existed and still numbered `record_hand`'s result `Result_1`. The `.did`
  next to it has been reconciled; the `.did.js` has not.
* `didc` is not installed on this machine, and hand-editing an `idlFactory` for a canister that
  custodies nothing but is the sole durable record is a change that should be generated, not
  typed.

<a id="e-69"></a>
### E-69, critical, every money door moved real money before writing down that it was going to — STATUS: FIXED (wave 8)

The full evidence, the eleven-door recovery sweep, the four failed attempts at forcing a literal
trap and the design of the fix are in
**[SECURITY-FINDINGS.md FINDING 29](SECURITY-FINDINGS.md#finding-29)**. The short version, because
the shape generalises and the next person writing an inter-canister call in this repo needs it:

> **An IC message is atomic only up to each await.** Everything a method writes BEFORE its first
> `await` is committed when the call goes out. Everything it writes AFTER can be discarded — by a
> trap, by an instruction limit, by an upgrade that drops the callback, or (this one needs no fault
> at all) by an ordinary call-level `Err` that is not evidence the callee did nothing.
>
> So: **if a method moves money and then writes down that it moved money, the second half is
> optional and the first half is not.** Write the intent first, make the retry safe at the LEDGER
> rather than in your own bookkeeping, and give the record a resume path the owner can drive.

Three details worth carrying forward:

* **`Duplicate` is a positive answer.** ICRC-1 and ICRC-2 deduplicate on the whole transaction
  including `memo` and `created_at_time`. Re-issuing the identical transaction is how a canister
  finds out whether a movement it lost track of actually happened — and the answer arrives carrying
  the block index. That is why `transfer_tokens()`, which sent `memo: None` and
  `created_at_time: None`, had to be **deleted** rather than left unused: a transaction the ledger
  cannot deduplicate is a transaction you can never safely retry.
* **A lease is not a safety mechanism.** Two callers driving the same recovery cannot double-move
  (the ledger deduplicates) or double-credit (the journal removal is atomic). The lease only stops
  them paying for the same round trip, so it is 30 s. A long lease would be a second lock for a
  discarded continuation to get stuck behind — the defect wearing a hat.
* **It found a defect in another agent's wave-7 work.** [FINDING 28](SECURITY-FINDINGS.md#finding-28)
  records what the ledger last said about each deposit subaccount; a sweep's reset of that
  observation lives in the continuation. With both terms summed and the continuation discarded the
  canister counted the same e8s twice, and netting by the sweep amount alone still left the burned
  transfer fee being claimed after it had ceased to exist. `observed_deposit_total()` nets open
  sweeps **gross of fee** now. M14 was red until both halves were right, which is the whole argument
  for a gate that reads the chain and the books in one breath.

<a id="e-68"></a>
### E-68, medium, an upgrade destroys every hand the archive has not acknowledged yet — STATUS: OPEN

`UNRECORDED_HANDS` holds up to 64 complete `HandHistoryRecord`s that the archive did not
acknowledge — the whole point of the T-34 fix, so a table that could not reach the archive keeps
the proof and `flush_unrecorded_hands` can push it later. It is a bare `thread_local` and it is
**not a field of `PersistentState`**.

So `pre_upgrade` does not save it and `post_upgrade` restores an empty backlog. Upgrade a table
whose archive is unconfigured, unreachable or unauthorised — exactly the state the backlog exists
for — and every shuffle proof in it is gone, permanently, with `get_history_status` then honestly
reporting a backlog of zero.

Not fixed here because it is in the upgrade path, which had four editors this wave. The fix is one
`opt` field (`unrecorded_hands: Option<Vec<HandHistoryRecord>>`) written and read beside
`hand_history`, bounded at `MAX_UNRECORDED_HANDS`, and a test that upgrades a table with a
non-empty backlog and asserts the count survives. Found while re-anchoring the record for
[FINDING 30](SECURITY-FINDINGS.md#finding-30).

---

<a id="e-70"></a>
### E-70, high, THE FIFTH CROSS-AGENT DEFECT: every observation instrument was built for the deposit subaccounts and none for the main account — STATUS: FIXED (wave 10)

**CLOSED.** [SECURITY-FINDINGS.md FINDING 35](SECURITY-FINDINGS.md#finding-35) carries the fix,
what it does not fix, and the driven output. In one line: the main account now has an
observation record (`MAIN_CUSTODY`, persisted `opt` at the top level and verified across a real
`--mode upgrade`), anybody including an anonymous caller can take a reading
(`refresh_solvency()`, `refresh_main_account_custody()`), `get_solvency()` publishes OWES against
HOLDS with a signed difference, a three-state verdict and the age of every reading — `null`, never
zero, where one has never been taken — and the two surfaces a player already reads carry it:
`get_custody_status().canister_solvency` and the refusal a withdrawal gets when the ledger will
not pay it. Gated by `tests/money_safety/tests/solvency.rs` (9 tests) and by
`invariants::solvency` on every fuzz step, at a new never-excusable severity
`InsolvencyUnreported`.

**THERE IS A LIVE 2.00 ICP SHORTFALL ON MAINNET table_1 AND THIS DOES NOT FIX IT.** It makes it
visible and quotable. No setter was added and none may be:
`no_setter_was_added_to_fix_the_books` is a source-level gate.

The shape, kept because the shape is what recurs:

Five agents edited `src/table_canister/src/lib.rs`. The wave's organising insight, *a query
cannot call the ledger, so ask from an update and write the answer down*, was implemented for
accounts (2) and (4) of THE ACCOUNT CENSUS and for nothing else. The main accounts, (1) and (3),
where essentially all the money is, have no observation record, no reader and no term in
`total_liability()`. So `admin_audit_deposit_custody` replies `(1 audited, 0 held, 0 unaudited)`
on a canister holding 5 ICP, and `admin_update_config(BTC)` is accepted on the same canister.

It is not the "correct totals, wrong recipients" signature this time. It is the one above it:
**a correct instrument pointed at a subset of the accounts, and a census whose MEASURED BY column
names the test harness rather than the canister.**

**WAVE 9: THE PLAYER-FACING HALF IS BUILT, AND IT IS BLOCKED ON ONE GENERATED FILE.**

The canister half landed during this wave: `src/table_canister/table_canister.did` now publishes
`get_solvency : () -> (SolvencyReport) query` and `refresh_solvency : () -> (Result_Solvency)`,
with a `SolvencyVerdict` variant of `CanPayEveryone | CannotPayEveryone | Unknown`.

**`src/declarations/table_1/table_1.did.js` HAS NOT BEEN REGENERATED FROM IT.** That generated
file is the Candid the BUNDLE is compiled against, so as of this build the browser does not know
those methods exist, `describeSolvencySurface()` finds nothing, and the deposit screen renders
`unsupported`. The wording is still true, and it is still a warning rather than silence — but the
canister can now answer and the UI is not asking. Regenerating that one file is the whole
remaining step; nothing in `src/cleardeck_frontend/**` needs to change, which is the property the
discovery-by-Candid design was for. `tools/shots/test-solvency.mjs` asserts BOTH halves: the
shipped `SolvencyReport` field-for-field (so the frontend is proven ready), and that today's
declarations carry no solvency surface (so this paragraph cannot go stale silently).

What the frontend does with each answer:

* `src/cleardeck_frontend/src/lib/solvency.js` DISCOVERS the solvency surface from the table's
  own Candid (`declarations/table_1/table_1.did.js`), structurally: a query whose return record
  carries both "what the ledger holds" and "what is owed", or a stated shortfall. It names no
  method, so regenerating the declarations wires it with no edit here.
* **`verdict` WINS OVER ANY ARITHMETIC THIS CLIENT COULD DO**, because the Candid says so:
  *"shortfall_e8s is null when there is no shortfall OR when the answer is not known — branch on
  `verdict`, never on this"*. Deriving from `shortfall_e8s` would read UNKNOWN as FINE. An
  unrecognised verdict tag degrades to `unknown`, never to `covered`. And `as_of_ns` (when the
  REPORT was computed, always set) is deliberately NOT matched as an observation timestamp; only
  `main_observed_at_ns` is, because matching the wrong one turns "nobody has ever looked" into
  "looked just now". All three are pinned by tests.
* Until the declarations land the discovery returns nothing and the state is `unsupported`, which renders as
  **"This table cannot report whether it actually holds the money it says it owes"**, in red,
  above every control that can move money, on the deposit screen and the withdraw screen.
  `covered` is the only state that renders nothing. **Unknown is not zero and unknown is not
  fine** — the four other states are all reasons not to deposit, and they are worded that way.
* `src/cleardeck_frontend/src/lib/components/SolvencyNotice.svelte` is in the document flow with
  no `position: fixed` and no z-index, so it cannot become one more thing that covers the four
  protected notices ([E-52](#e-52)).

This does not close E-70. A UI that says "the canister cannot tell you" is the correct rendering
of an open defect, not a fix for it. What it removes is the state the finding is actually about:
a player standing in front of a deposit button on a table that is 2.00 ICP short, with the
application showing them nothing at all.

<a id="e-71"></a>
### E-71, high, the archive's `(table_id, hand_number)` is not a key: seventy records answer to "table_2 hand 1", with six different pots — STATUS: FIXED (wave 13)

Measured on the local archive after the wave-8 screenshot sweep
(`icp canister call history get_recent_hands '(20)' --query`):

```
hand_id 37  table 4xhad(table_2)  hand_number 1  total_pot 2_400_000_000
hand_id 36  table 4xhad           hand_number 1  total_pot 2_400_000_000
hand_id 30  table 4xhad           hand_number 1  total_pot 2_400_000_000
hand_id 24  table 4xhad           hand_number 1  total_pot    20_000_000
hand_id 22  table 4xhad           hand_number 1  total_pot    20_000_000
hand_id 20  table 4xhad           hand_number 1  total_pot    20_000_000
...
```

`hand_number` restarts at 1 every time a table is reset or reinstalled, and the archive is
append-only with no uniqueness constraint on `(table_id, hand_number)`. Nothing in
`record_hand` rejects or distinguishes a repeat.

**Why this matters more than it looks.** `hand_number` is the identifier
[SHUFFLE-SPEC.md](SHUFFLE-SPEC.md) and the hand-history UI use to name a hand. A verifier who
asks the archive for "table_2 hand 1" gets an arbitrary one of eleven records with three
different pots. This is what the screenshot sweep's HISTORY CHAIN DISAGREEMENT is:

```
hand 1: the TABLE canister paid 2400000000 e8s but the HISTORY canister recorded 5010000000 e8s
```

The number on screen and the number in the archive are both true of *a* hand 1 and they are not
the same hand 1. [FINDING 30](SECURITY-FINDINGS.md#finding-30) made the CONTENT of each record
true; it did not give the records identity.

---

## E-71 — how it was closed (wave 13)

### 1. The damage, counted rather than sampled

Every record in the local archive, read back and recounted on 2026-08-08. The wave-8 note above
sampled twenty; this is all 3,218:

```
distinct (table_id, hand_number) citations : 1651
records                                    : 3218
citations that name MORE THAN ONE record   :  947
records living under such a citation       : 2514      <- 78% of the archive
citations whose records have DIFFERENT pots:  877
worst: table_2 "hand 1" -> 70 records, 6 distinct pots
```

**And a verifier did get the wrong hand back, silently.** The screenshot sweep's history-chain gate
fetched the newest twenty records for `table_2` and put them in a `Map` keyed on `hand_number`.
Twenty records collapsed to **ten** keys. The record left under "hand 1" was `hand_id 3173`, pot
0.2 ICP, from an earlier life of the table; the hand just played was `hand_id 3218`, pot 24 ICP. The
gate then reported *"the TABLE canister paid 2400000000 e8s but the HISTORY canister recorded
20000000 e8s paid"* — which reads as the archive losing 22 ICP and is really the gate comparing two
different hands. Nothing anywhere said the two records were not the same hand.

The ten records the map discarded were **also** the ten it never checked for a rake, because the
no-rake loop iterated the same collapsed map. Half the evidence for the product's headline property,
dropped by insertion order, on the one gate that asserts it against the permanent archive.

### 2. What names a hand now

```
hand_uid = "<table canister id>:<seed_hash>"
```

The shuffle commitment. It is 32 bytes of `raw_rand` per hand, published before any card is shown,
already carried by every record ever archived, and already on the player's own screen while the hand
is running — the one value in a hand record that is unique to the hand by construction rather than by
bookkeeping. Recounted over the same 3,218 records: **3,218 distinct commitments, 0 empty, 0 that
are not 64 lowercase hex.** It was already a perfect key; nothing was using it as one.

The table is in the name for what happens when the commitment is *not* unique. Keyed on the
commitment alone, a second table sending a record that reused another table's commitment would be
de-duplicated away — a genuine hand discarded with an `Ok`, which is [E-49](#e-49) exactly. Scoped to
the table, both records survive and the collision is visible in `get_archive_integrity`.

It is **derived on read, never stored and never accepted from a writer**. `record_hand` clears
whatever a caller put in the field; `get_hand`, `get_hands_by_table`, `get_hands_by_player`,
`get_recent_hands` and `check_recorded_hand` all fill it from `table_id` and
`shuffle_proof.seed_hash` through one function. That is the whole migration.

### 3. Migration — exactly what happened to the records that already exist

Nothing was rewritten, renumbered, reindexed or moved.

| | |
|---|---|
| `hand_id` | unchanged. Still the archive's primary key, still what `get_hand` takes, still what five auditors' citations resolve through |
| `hand_number` | unchanged. Still recorded, still returned, still not a name |
| every stored byte | unchanged. `hand_uid` is `opt` in storage ([FINDING 14](SECURITY-FINDINGS.md#finding-14)) and is **never written**, so old and new records are identical on disk and are named by the same three lines of code |
| the de-duplication key | narrowed from `(table_id, hand_number, seed_hash)` to `(table_id, seed_hash)`. Over the existing archive this merges nothing: 3,218 records, 3,218 distinct commitments |
| an old `(table, hand number)` citation | still resolves — `resolve_hand_number` returns the whole SET it always named, oldest first, each with its pot, timestamp and permanent name, plus `is_unique` and advice. The number the auditor quoted pins which one they meant |

**Proved on the live local archive, not argued.** All 3,218 records were snapshotted field by field,
the canister was upgraded (`--mode upgrade`), and the snapshot retaken:

```
records present before and missing after : 0
records that appeared                    : 0
records whose CONTENT changed            : 0
```

then, on the upgraded canister, every record written under the old scheme:

```
get_archive_integrity ->
  records_held                            3218
  records_with_a_name                     3218
  distinct_names                          3218
  name_collisions                            0
  records_without_a_usable_commitment        0
  index_disagrees_with_records           false
  ambiguous_hand_number_citations          947
  worst_hand_number_citation   4fbx2… hand number 1 answers to 70 records
resolve_hand_number(table_2, 1) -> 70 matches, is_unique=false,
  and all 70 resolve back to THEMSELVES by name (70/70)
```

`get_archive_integrity` recounts from the RECORDS, not from the name index — an index cannot be
used to check itself, and an index that disagrees with its records is the failure this canister is
most likely to have. It is also how the property can be checked on an archive nobody is allowed to
call from a test.

**About the MAINNET archive, stated exactly.** It was not called: this wave is local-only and
`kggj7-…` is off limits, so nothing here is a measurement of it. What is claimed instead is
structural and checkable by anyone in one call. The name is a pure function of `table_id` and
`shuffle_proof.seed_hash`, two fields every record on every instance already carries, so installing
this code names whatever is there without writing to it; the `opt` field means `stable_restore`
accepts state written before it existed; and `get_archive_integrity` reports, for *that* instance,
whether the naming actually holds — `name_collisions`, `records_without_a_usable_commitment` and
`index_disagrees_with_records`. The local instance is the evidence that the query is honest about a
real archive: it reports the 947 ambiguous citations rather than a clean sheet, and it was caught
being blind once already (see §5). Run it after the upgrade rather than trusting this paragraph.

### 4. A record with no usable commitment is REFUSED, not stored

`record_hand` returns an `Err` naming the problem. Storing such a record would mean every unnamable
record from one table sharing one name and the second being de-duplicated into the first — a genuine
hand discarded with an `Ok`, [E-49](#e-49) again. Refusing is loud: the table keeps the record in its
retry backlog and `get_history_status` reports the sentence verbatim. No hand the table canister
produces can hit this (it returns before building a record when there is no shuffle proof), and all
3,218 existing records carry a 64-hex commitment.

### 5. The gate, and the blind spot it was hiding

`tools/shots/lib/chain-agreement.mjs` now joins the table's hands to the archive **by name**, and —
the part that matters — **asserts its own join**: whatever record comes back must carry the name of
the hand it is being compared against. A future edit that puts the lookup back on `hand_number` fails
on the join, naming both hands, instead of producing a plausible money mismatch. Reverted end to end
on the live replica, the `handhistory` shot goes red with

```
JOIN BROKEN: hand 1 of this table committed to 4fbx2…:c12ece47…, but the archived record being
compared against it is 4fbx2…:c9952d42… (hand_id 3201, pot 20000000). Those are two different hands.
```

Four more holes closed while in there:

* the no-rake loop iterates the records fetched, not the collapsed map, so all twenty are checked
  rather than ten;
* **no-rake is now asserted over the WHOLE archive**, not over the window the gate can afford to
  fetch. `get_archive_integrity` carries `records_with_a_nonzero_rake` and `rake_recorded_total`,
  recounted from every record on every call, so a rake no longer only has to wait twenty hands to be
  checked by nothing at all;
* the two sides' `hand_number`s are now COMPARED, which was impossible while the number was the join
  key — the join made them equal by construction. One deal recorded under two different numbers is
  the two surfaces disagreeing about the record they hold, and it now fails;
* the gate reads the archive and the table's hand record through the harness's **own** Candid
  mirrors (`lib/archive-wire.mjs`, new; `lib/hand-record-wire.mjs`), because
  `src/declarations/history/history.did.js` is stale ([E-67](#e-67)) and would have dropped
  `hand_uid` in silence — sending the join quietly back to `hand_number`, which is the defect.

`tools/shots/test-hand-identity.mjs` holds all of it with no replica, no browser and no canister:
25 cases, including the eleven-hands-one-number archive, the injected by-`hand_number` lookup, a
truncated commitment, a name that does not follow from its own record, and a `HandSummary` with the
field dropped.

**And the instrument was blind in the way this project's standing lesson predicts.**
`index_disagrees_with_records` was first written as `hand_uid_index.len() != distinct_names`, which is
self-confirming: two records sharing a name produce ONE name and ONE index entry, the two agree, and
the field that exists to report exactly that drift cannot see it. Caught by
`the_integrity_report_sees_a_collision_the_index_would_hide`, which forces a twin in behind
`insert_hand`'s back — the test failed on the field, not on the archive. It compares the index with
the number of NAMED RECORDS now.

### 6. A hand rebuilt from a record, by name, to check nothing was lost

`hand_uid` `4xhad-gd777-77775-aaacq-cai:ecc34b2f1d5325b7c982b8ed7ed19bc411d427b9b1daecc8857c9ecc23f9dc58`
was fetched with `get_hand_by_uid`, and its seed handed to the Python verifier written from
[SHUFFLE-SPEC.md](SHUFFLE-SPEC.md), which shares no line with the canister:

```
P = 4 (dealt_in states it)     board  derived [Qd 8c Ad 9s 2d]  record [Qd 8c Ad 9s 2d]  MATCH
seat 0  derived [Ks 9c]  record [Ks 9c]  MATCH
seat 1  derived [Tc 8d]  record [Tc 8d]  MATCH
```

Seats 2 and 3 folded and their cards are not in the record; the verifier derives them anyway
(`8h Th`, `Qc Ac`) from the seed alone, which is the property an auditor used to reconstruct a folded
player's hand. Nothing the record carries changed, so that still holds.

### 7. Also fixed, in `record_hand_to_history`: the record could carry another hand's seed

`seed_hash` came from `state.shuffle_proof` (this hand's) and `revealed_seed` came from
`HAND_HISTORY.last()` (whatever is on the end of the ring). `reveal_seed_on_hand_end` writes the seed
into the entry matching **(hand_number, seed_hash)**; this read it off the end. When the two are not
the same entry, the archive is handed this hand's commitment beside another hand's seed,
`check_recorded_hand` finds they do not hash to each other, and a permanent append-only record
accuses the table of revealing a seed it did not commit to. Latent — all 3,218 archived records hash
correctly — and now structurally impossible: the read uses the same predicate as the write, so the
seed in a record can only ever be the pre-image of the hash in the same record. A miss leaves
`revealed_seed` empty, which `check_recorded_hand` already reports in its own words instead of
manufacturing an accusation.

<a id="e-72"></a>
### E-72, high, the money-safety harness's `CustodyStatus` mirror silently drops `unfinished_ledger_ops` — STATUS: FIXED (wave 10)

**CLOSED.** The field is declared, and — the part that matters —
`CustodyStatus::components_sum_to_total()` asserts an identity that cannot hold unless EVERY
component field is declared, so the next silently dropped field fails a test instead of
narrowing the record in silence. [SECURITY-FINDINGS.md FINDING 36](SECURITY-FINDINGS.md#finding-36).

[SECURITY-FINDINGS.md FINDING 36](SECURITY-FINDINGS.md#finding-36). Cross-agent 1 × 2: agent one
wrote the comment *"Mirrored in FULL on purpose"*, agent two added a tenth field to the canister's
record, Candid record subtyping dropped it without a warning, and every harness assertion against
the custody surface has been reading nine of ten fields ever since. Found by writing the field
name in a probe and having `rustc` refuse it.

<a id="e-73"></a>
### E-73, high, `total_liability` — the last custody guard's only input — has one caller, no query and no gate — STATUS: FIXED (wave 10)

**CLOSED by the one query this entry asked for.** `get_solvency().guard_liability` IS
`total_liability()`, published beside every term it is built from, and
`invariants::solvency::check_solvency_report_is_coherent` asserts all of them term by term
against independently computed figures on every snapshot the harness takes — including every
fuzz step. A term that goes missing from the guard now fails a test instead of waiting for
somebody to flip a currency and find out.
[SECURITY-FINDINGS.md FINDING 37](SECURITY-FINDINGS.md#finding-37).

<a id="e-74"></a>
### E-74, medium, `local-up` reports success for a lobby it did not populate, and an empty lobby makes the ENTIRE screenshot harness unrunnable — STATUS: OPEN

The fourth auditor filed the symptom as low severity ("✓ lobby lists 0 table record(s)" printed as
a pass). Measured this wave: it is the reason `./scripts/dev.sh shots` could not run.

```
[5/6] resolve lobby table names
  lobby lists 0 tables; mapped: {}
...
  ✗ FAILED: Lobby has no registered name for table_2      (x20)
12 scene(s) not verified   EXIT=1
```

`scripts/dev.sh` calls `lobby init_microstakes_tables` with `--identity "$CONTROLLER"` and
discards a non-zero reply with `|| warn`. On this instance the lobby's admin is a different
principal, so the call returns `Err("Only admin can initialize tables")`, the script prints a
green tick on a count of zero, and every downstream harness that resolves a table by name dies.

Running the identical call as the identity `lobby get_admin` actually names returns
`(variant { Ok })` and the lobby lists 3 tables; the sweep then completes and verifies 17 of 24
shots. **Two waves of "the pixel gate cannot run" were this.** The fix is the one `dev.sh`
already applies to the archive wiring one function above: read the result back and fail, plus
resolve the identity from `get_admin` rather than assuming `$CONTROLLER`.

<a id="e-75"></a>
### E-75, medium, a player's stack figure is painted over by the pod clock at 390x844 — STATUS: OPEN

```
table-sidepots [mobile]
  OCCLUSION FAILED: 1 of 26 figures on screen are covered —
  9.8% of "50.00" (span.chips.svelte-1dy35r9) by span.pod-clock.svelte-1dy35r9
```

Same class as [E-63](#e-63) and a different pair of elements: E-63 was the action dock over the
felt and was fixed by sizing the dock to its contents; this is the action clock badge inside a
player pod over that pod's own stack figure, in the ring-crowded 9-pod layout. First seen because
this is the first sweep in three waves that could reach a table scene at all ([E-74](#e-74)).

<a id="e-76"></a>
### E-76, medium, the felt drops below its floor on three of the twenty-four shots — STATUS: OPEN

```
table-preflop     [desktop] 27.8% of 1440x900 (870x414.3)    floor 28%
table-facing-bet  [desktop] 27.8% of 1440x900 (870x414.3)    floor 28%
table-allin       [mobile]  42.8% of 390x844  (279.5x503.6)  floor 45%
```

The felt-area floor exists because a playing surface that shrinks while the protected notices stay
on screen is the regression no other gate can see (WAVE-05). All three are marginal, 0.2 and 2.2
points, and all three are real. The two desktop rows are the action dock's height in the 6-pod
default layout; the mobile row is the ring-crowded 9-pod layout, the same layout as
[E-75](#e-75).

---

## Found by the wave-11 register pass

*The job was to make this register TRUE. Verifying the closed entries found four defects, and
three of them are the same defect: **a gate that exists, passes, and is invoked by nothing.**
That is [H-17](#h-17), which this project closed in wave 3 with the sentence "the fund-theft
gate was outside the gate", recurring at four times the scale on the two most recent
cross-agent findings.*

<a id="h-55"></a>
### H-55 — high — the drift guard compared one field in eight and said "matches" — STATUS: FIXED (wave 14)

**Found 2026-08-09, while adding [L-04](#l-04)'s lobby leg to the same script.**

`scripts/check-deployed-config.sh` is the answer to [D-12](#d-12): it reads each table's live
`TableConfig` off the canister and diffs it against `icp.yaml`'s `init_args`, field by field. It
exists because `btc_table_1` ran on blinds of 10/20 sats and a buy-in range of 1,000-20,000 for its
entire life while `icp.yaml` declared 100/200 and 10,000-100,000 — ten times larger across every
field, through nine upgrades and four audits, and found by a player-facing strikethrough rather
than by us.

It compared one field. This was the extractor:

```sh
got="$(printf '%s' "$live_raw" | tr ';' '\n' \
      | grep -E "^[[:space:]]*$f = " | head -1 | grep -oE '[0-9_]+' | head -1 | tr -d '_')"
[ -n "$got" ] || continue
```

`[0-9_]+` matches the underscore **inside the field name**. On the real line

```text
      small_blind = 5_000_000 : nat64
```

the matches are `_`, `5_000_000`, `64` — in that order — so `head -1` takes the underscore from
`small_blind`, `tr -d '_'` turns it into the empty string, and the caller's
`[ -n "$got" ] || continue` skips the field without a word. Measured, field by field:

```text
  small_blind            extracted=[] <- SKIPPED, never compared
  big_blind              extracted=[] <- SKIPPED, never compared
  min_buy_in             extracted=[] <- SKIPPED, never compared
  max_buy_in             extracted=[] <- SKIPPED, never compared
  max_players            extracted=[] <- SKIPPED, never compared
  action_timeout_secs    extracted=[] <- SKIPPED, never compared
  ante                   extracted=[5000000]
  time_bank_secs         extracted=[] <- SKIPPED, never compared
```

`ante` is the only name in `TableConfig` with no underscore in it.

**The conviction, using the guard's own subject.** `table_2`'s contract was moved to
`action_timeout_secs = 60` with `admin_update_config` while `icp.yaml` declares 45:

```text
before   ✓ table_2 (4fbx2-kt777-77775-aaabq-cai) matches icp.yaml
after    ✗ table_2 (4fbx2-kt777-77775-aaabq-cai)
             action_timeout_secs: declared 45, running 60
```

**Fixed** by one `field_value()` helper — `sed -E 's/^[^=]*=[[:space:]]*([0-9_]+).*$/\1/'`, which
reads the value *after the equals sign* — used by both comparisons, and a `--selftest` that reads
all eight fields out of a sample reply, requires an absent field to read as empty rather than as
`0` (so "cannot read" stays distinguishable from "read a zero"), and requires a 5x drift not to
compare equal. A caller that now cannot read a field says so and fails instead of continuing.

This is the standing lesson in the instrument rather than the canister: *an instrument that
measures nothing passes*. Nothing about the output changed when it went blind, because a green
tick over zero comparisons looks exactly like a green tick over eight.

<a id="t-45"></a>
### T-45 — medium — the local gateway port is hardcoded a third time, in the wallet path — STATUS: OPEN

**Found 2026-08-09 while fixing [T-14](#t-14),** by grepping the built bundle for `4943` after the
fix and finding it still there.

```text
src/cleardeck_frontend/src/lib/oisy.js:60   const host = isMainnet() ? IC_HOST : 'http://localhost:4943';
src/cleardeck_frontend/src/lib/oisy.js:165  const host = isMainnet() ? IC_HOST : 'http://localhost:4943';
src/cleardeck_frontend/src/lib/oisy.js:301  const host = isMainnet() ? IC_HOST : 'http://127.0.0.1:4943';
```

[T-03](#t-03) removed this from `ic-config.js`, [T-14](#t-14) removed it from the build environment
that feeds it, and `auth.js` derives the local Internet Identity origin from the same value. The
OISY wallet path never joined in, so connecting an external wallet against the local replica still
points at a port this project does not run. The fix is `LOCAL_HOST` from `ic-config.js`, which is
already imported by its neighbours.

Not fixed here: `src/cleardeck_frontend/**` is another owner's tree.

<a id="t-46"></a>
### T-46 — high — the mainnet bundle verifier's network detector no longer matches what vite emits — STATUS: OPEN

**Found 2026-08-09 while trying to close [H-46](#h-46)** (the verifier's own self-test is invoked by
nothing, so the first step was to run it, and it could not get past the unmutated bundle).

```text
$ npm --workspace src/cleardeck_frontend run build:mainnet
  ✗ compiled network is "ic"    no network literal compiled in: the bundle would fall back to
                                sniffing window.location.hostname, which is exactly the guess
                                docs/DEFECTS.md T-01 forbids
npm error Lifecycle script `build:mainnet` failed with error: code 1
```

`build/verify-bundle.mjs` finds the compiled target by anchoring on `MAINNET_HOSTNAMES` and then
matching the function immediately after it:

```js
const m = window.match(/function\s+\w+\(\)\{return"(ic|local)"\}/);
```

That shape came from

```js
function compiledNetwork() {
  const raw = import.meta.env.VITE_ICP_NETWORK || import.meta.env.DFX_NETWORK;
```

which minifies to a bare `return"ic"`. Wave 14 rewrote it as

```js
const raw = buildValue(() => import.meta.env.VITE_ICP_NETWORK)
  || buildValue(() => import.meta.env.DFX_NETWORK)
  || buildValue(() => process.env.DFX_NETWORK);
```

for a good reason ([FINDING 42](SECURITY-FINDINGS.md#finding-42)'s trusted-table set must be
loadable outside vite), and the emitted function is now a call through a helper rather than a
literal return. **The bundle is almost certainly still correct** — vite's `define` substitutes
`import.meta.env.X` textually wherever it appears, including inside an arrow function — so what has
broken is the DETECTOR, and that is the worse of the two possibilities: the only gate on
[T-01](#t-01) now fails on every mainnet build, which is exactly the pressure that got the previous
Candid drift job muted behind an `exit 0`.

**Not fixed here:** `src/cleardeck_frontend/**` is another owner's tree, and the fix is a judgement
about what the detector should anchor on (the safest is to stop pattern-matching minified output and
have the build emit the target as a named export the verifier can read).

**Blocks [H-46](#h-46):** the verifier's self-test cannot be wired into CI until the verifier passes
on an unmutated bundle, because its first act is to check exactly that.

<a id="h-45"></a>
### H-45 — high — four money-safety gate targets are named by no target, and they are the gates of the FOURTH and FIFTH cross-agent defects — STATUS: FIXED (wave 14)

> **FIXED 2026-08-09, and the fix is not the six lines — it is that the question is now asked by a
> machine, first, before anything else runs.** All six targets have been named in
> `scripts/dev.sh cmd_test` since commit `134550e`; that closed the instance and not the class,
> which is what the previous six recurrences also did.
>
> Step **[1/9]** of `./scripts/dev.sh test` is now `./scripts/check-suite-wiring.sh`, and it
> `die`s rather than collecting a failure: a tree containing a test target nothing names does not
> get to run its gates at all.
>
> **RED, PLANTED.** An empty `tests/money_safety/tests/zz_planted_unwired.rs`:
>
> ```text
> $ ./scripts/dev.sh test          # 8 seconds
> ==> [1/9] the gates' own wiring (docs/DEFECTS.md H-45, D-11)
>     FAIL  tests/money_safety|zz_planted_unwired is a cargo test target that NOTHING NAMES
> FATAL: suite wiring is broken -- a test target in this tree is run by nothing.
> $ echo $?
> 1
> ```
>
> **RED, UNPLANTED, DURING THIS WAVE.** The first time the step was run for real it went red on
> `tests/money_safety/tests/solvency_definition.rs` — the gate on
> [FINDING 43](SECURITY-FINDINGS.md#finding-43)/[38](SECURITY-FINDINGS.md#finding-38), written by
> another owner minutes earlier and named by nothing. That is **H-45 for the SEVENTH time**, caught
> in a second rather than in a wave. It is wired now, in `scripts/test-suites.list` and in
> `cmd_test`, last in the chain so it can hide nothing.
>
> **THE SECOND HALF, which no earlier wave named.** `cmd_test`'s money-safety block was a single
> `&&` list, so **one red target skipped the fifteen after it**. Measured on this tree the same
> day: a single failing assertion in `deposit_floor` (target 5 of 18) ended the step in 131 seconds
> having never run `solvency`, `stall_agreement`, `fund_reachability`, `controller_custody` or
> either fuzz invocation. "Run by nothing" and "skipped because something earlier failed" are the
> same hole, and the second one has a green tick further up the log. Every target now runs through
> `run_ms`, which records a failure and carries on, and the step reports the complete list.

**Status: executed 2026-08-06.** Method: every `--test <name>` in `scripts/`, `Makefile` and
`.github/workflows/` was extracted and compared against `ls tests/money_safety/tests/*.rs`.

```
                              named by a runnable target?
invariants                    yes   (scripts/dev.sh cmd_test)
regressions                   yes
deposit_replay                yes
ui_limits                     yes
deposit_floor                 yes
admin_custody                 yes
ledger_boundary               yes
coherence_w8                  yes
wave6_coherence               yes
timers                        yes
fuzz                          yes
--------------------------------------------------------------------
stall_agreement               NO     2 tests   M13 ONE BELIEF
solvency                      NO    12 tests   the FINDING 35/36 gate
deposit_subaccount_anchor     NO    11 tests   the FINDING 28/11/21 gate
fund_reachability             NO     6 tests   M9's own file   -- and RED, see H-48
oldest_cluster                NO     6 tests   the FINDING 22 / E-78 / E-79 gate   (new, wave 11)
deposit_surface               NO     9 tests                                       (new, wave 11)
```

**46 tests in six files, run by nothing — and it GREW BY TWO FILES DURING THIS PASS.** The list
was four files when this entry was written; `oldest_cluster` and `deposit_surface` were added by
other owners in the same wave, and `oldest_cluster` is the gate named by the [E-78](#e-78) and
[E-79](#e-79) rows in [THE REGISTER](#the-register). This is not a backlog that stopped growing
when somebody noticed it. Reproduce the measurement with:

```sh
ls tests/money_safety/tests/*.rs | xargs -n1 basename | sed 's/.rs$//' | while read t; do
  grep -rq -- "--test $t\b" scripts Makefile .github || echo "UNWIRED: $t"
done
```

**Four files, at the time this entry was written:** `scripts/dev.sh cmd_test` names its money-safety
targets one at a time, and the comment above the list says exactly why:

> *`deposit_replay` carries the E-02 FUND-THEFT reproducer and the ten regressions that keep it
> shut. It is a cargo-auto-discovered target, so for the whole of wave 2 it was named by NO make
> target and run by nobody: the project's only proven fund-theft primitive had its gate outside
> the gate. Named explicitly here so that cannot recur silently.*

It recurred silently. Four times, in the four waves after that comment was written.

**What each unrun file is the gate for, and what the register was therefore claiming:**

| file | claimed to gate | register said |
|---|---|---|
| `stall_agreement.rs` | **M13 ONE BELIEF** — [E-59](#e-59) / [FINDING 25](SECURITY-FINDINGS.md#finding-25), the FOURTH cross-agent defect. 138 forked states; 118 rows paying different recipients → 0 | "Gated by **M13 ONE BELIEF**" |
| `solvency.rs` | [E-70](#e-70) / [E-72](#e-72) / [FINDING 35](SECURITY-FINDINGS.md#finding-35) / [36](SECURITY-FINDINGS.md#finding-36), the FIFTH. Includes `CustodyStatus::components_sum_to_total`, **the only assertion in the tree that stops the harness's custody mirror silently dropping a field again** | "gated by `tests/money_safety/tests/solvency.rs` (12 tests)" |
| `deposit_subaccount_anchor.rs` | [FINDING 28](SECURITY-FINDINGS.md#finding-28) (money at the canister's own published address invisible on every surface), [FINDING 11](SECURITY-FINDINGS.md#finding-11) / [E-12](#e-12), [FINDING 21](SECURITY-FINDINGS.md#finding-21) | the findings read as closed |
| `fund_reachability.rs` | **M9 FUND REACHABILITY**, the auditor's fund lock | — |

**The three claims are not equally wrong, and the difference matters.**

* [E-70](#e-70) / [E-73](#e-73) are genuinely gated, by a *different* mechanism the register also
  names: `invariants::solvency` runs on **every fuzz step** (`fuzz.rs`
  `check_insolvency_is_reported`, `check_solvency_report_is_coherent`) and `dev.sh test` runs the
  fuzzer twice. Half of the claim is true. The `solvency.rs` half is not.
* [E-72](#e-72) is **not** gated by anything that runs. `components_sum_to_total()` is called in
  exactly two places, both in `solvency.rs`.
* [E-59](#e-59) is **not** gated by anything that runs. M13 lives only in `stall_agreement.rs`.

**Not fixed here.** `scripts/dev.sh` is another owner's file and adding four targets to the
project's primary gate changes what green means; that is a decision, not a cleanup. The
one-line-per-file change is the same shape as the `deposit_replay` line already in `cmd_test`.

Until it lands, every `FIXED` row in [THE REGISTER](#the-register) whose gate column contains
**`NOT RUN`** is a fix held by a test nobody runs. `./scripts/register-stats.sh` prints that list
under "GATE EXISTS BUT NO TARGET RUNS IT".

<a id="h-46"></a>
### H-46 — medium — the mainnet-bundle verifier's mutation self-test is invoked by nothing — STATUS: OPEN

**Status: executed 2026-08-06.** `src/cleardeck_frontend/build/verify-bundle.selftest.mjs` exists,
carries 8 planted mutations, and appears in no npm script, no make target, no `scripts/dev.sh`
command and no CI job:

```
$ python3 -c "import json;print(json.load(open('src/cleardeck_frontend/package.json'))['scripts'])"
{'setup': …, 'start': …, 'build': …, 'build:mainnet': …, 'verify:bundle': …,
 'verify:deployed': …, 'check': …, 'format': …}
```

There is no `verify:bundle:selftest`.

This is the self-test that exists **because the first version of that verifier passed 12 of 12
while measuring nothing** — one wrong repetition count in a regex made `allPrincipals()` return an
empty map, and "no local replica canister id in a mainnet bundle" printed a green tick over
`checked 0 distinct canister id(s)` on a bundle containing eleven. The instrument written to stop
the standing lesson repeating is itself unreachable, which is the standing lesson repeating.

Same class as [H-45](#h-45) and [H-38](#h-38). Not fixed here: `src/cleardeck_frontend/**` is
another owner's tree.

<a id="h-47"></a>
### H-47 — low — the two lists of "the screenshot harness's own self-tests" have diverged — STATUS: FIXED (wave 12)

**Status: executed 2026-08-06.**

```
tools/shots/package.json  "selftest":
    test-occlusion  test-census  test-money  test-rake  test-solvency
scripts/dev.sh  cmd_shots_selftest  (this is what `./scripts/dev.sh test` runs):
    test-occlusion  test-census  test-money  test-rake  test-dock-overflow
```

`test-solvency.mjs` is in neither the primary gate nor `make test`; `test-dock-overflow.mjs` — the
[E-63](#e-63) gate — is not in the command `tools/shots`'s own package file offers. [H-38](#h-38)
was closed by wiring `cmd_shots_selftest` into `cmd_test`, and the wiring copied the list instead
of calling the one that already existed. Two lists, one of them wrong whichever way you look.

Low rather than medium only because both lists are short and both are green today; the shape is
[H-45](#h-45)'s.

### The fix (wave 12)

Wave 12 was about to make it three lists: `test-table-in-frame.mjs` ([H-51](#h-51)) would have
been added to `cmd_shots_selftest` and not to the package file. So there is now ONE runner,
`tools/shots/selftest.mjs`, and both callers invoke it.

It **discovers** every `tools/shots/test-*.mjs` — so a self-test added tomorrow is run without
anyone remembering to wire it — and it also **requires** a named set to exist, because discovery
answers "did anything get missed" and not "did anything disappear", and a deleted gate cannot be
discovered. 7 files, 7 run, including `test-solvency.mjs`, which the repo's primary gate had
never executed.

<a id="d-07"></a>
### D-07 — medium — the README tells a stranger the deployed code is unverified; the handover says it matches 6 of 6 — STATUS: OPEN

**Status: executed 2026-08-06, by reading both surfaces.** `README.md` §"Verify the Code" carries
a boxed warning:

> ### ⚠️ The mainnet canisters do not satisfy this yet
>
> **The modules currently deployed to the mainnet canister IDs listed above were built before it
> existed**, on a developer's laptop, with the old non-reproducible recipe. […] the hashes to
> **not** match, and the script to print `NOT VERIFIED`. […] Until that happens, treat the
> deployed mainnet code as **unverified**.

The wave-11 handover states the opposite: all six backend canisters and the frontend run this
tree, and the deployed module hashes match a reproducible build **6 of 6**.

This is [T-21](#t-21) exactly — *two trust surfaces of the same product making opposite claims
about the same fact* — on the page a stranger reads first, about the single claim
[T-33](#t-33) exists for. Only one of the two can be true and **whichever it is, the README is
wrong or the handover is**, so a reader who checks cannot tell which.

Not fixed here: `README.md` is another owner's file, the fix is a re-run of
`./scripts/verify-build.sh` against the deployed hashes, and the correct wording depends on that
result. Do NOT resolve it by deleting the box: if the deployment really does match, say so with
the hashes; if it does not, the box is the honest text and [T-33](#t-33) is not closed.

<a id="d-08"></a>
### D-08 — high — the committed Candid did not describe the deployed code, and it is published on-chain — STATUS: FIXED (wave 11)

**Status: measured, fixed, gated 2026-08-06.** [FINDING 03](SECURITY-FINDINGS.md) reported the
committed `table_canister.did` as 241 diff lines from the interface the wasm exports. Re-measured
against the current build it was **1,429 lines of `diff -u`** — but the raw line count is the
wrong number, and chasing it is what kept this open. Most of it is the extractor renumbering
`Result_N` aliases and reordering fields, which the wire does not care about.

Compared **structurally**, over fully-resolved method signatures, the real drift was:

| interface | structural differences |
|---|---|
| `table_canister.did` | **3** — `get_hand_history`, `get_table_state`, and `get_deposit_replay_state` missing entirely |
| `lobby_canister.did` | **5 methods, 1 root cause** — `currency : opt Currency` where the canister exports a bare `Currency` |
| `history_canister.did` | 0 |

Seven fields were missing from the table interface: `Player.sitting_out_since`,
`ActionRecord.phase`, `ActionRecord.amount`, `HandHistory.dealt_in`, `HandHistory.participants`,
`TableState.last_action` and `TableState.departed_stakes` — the last being the record that stops a
departed player's stake being silently reassigned to the deepest stack ([E-05](#e-05)).

**This is not a documentation defect.** `recipes/rust-reproducible.hbs` line 99 embeds the `.did`
verbatim as the module's **public `candid:service` metadata**. Confirmed by reading it back out of
the built artifact: `ic-wasm .icp/cache/artifacts/table_1 metadata candid:service` returned the
stale committed file byte-for-byte. Every deployed canister was **publishing a false description
of itself on-chain**, and that is what third-party tooling reads to build a client.

**The failure was silent, not loud.** The finding predicted clients would "fail to decode a real
reply". They do not. Tested with `@dfinity/candid`: a bare `Currency` decoded against `opt
Currency` yields `[{BTC:null}]`, so `cfg.currency.BTC` is `undefined` — no error; and a record
carrying `sitting_out_since` decoded against a type without it drops the field — no error. Correct
-looking replies, missing information, nothing raised anywhere. The house signature again.

Fixed by hand-editing the three `.did` files, **not** by regenerating them: the table interface
carries several hundred lines of hand-written documentation the extractor does not emit, and that
documentation ships on-chain in the same metadata section. Gated by
`scripts/check-candid.sh` (CI job *Candid interface drift*).

**⚠️ THE MODULE HASH MOVES.** Because the `.did` is a build input, correcting it changes every
module hash. Measured, same directory, only the `.did` differing:

```
lobby    0cf8d722714549ced2ba6fd60dd0eab5ac731a25b67607c33cadb856d457f6e0  ->  5c6bd6334cea82b68f26ecefa4f51342aae418283f1fdea1feead65b23997852
table_1  f175b647dfceba108fa25191c795acd4210fa4da52736217c821798ab740d1d6  ->  00a9aa66ca02338ec02334e1eafd1004bdbf3dede32323d83aa3f39aafff9e76
```

**Mainnet will not match `main` until the lead redeploys.** The two-path reproducibility guard
still passes with the corrected files (verified: byte-identical at two different absolute paths),
so this costs a redeploy, not a rebuild of the verification story.

<a id="d-09"></a>
### D-09 — high — the "Candid interface drift" CI job could not go red, and compared the wrong thing — STATUS: FIXED (wave 11)

**Status: root-caused and replaced 2026-08-06.** The job existed, ran on every PR, and passed for
the entire time [D-08](#d-08) was true. Its final line:

```sh
          echo "candid drift status: $fail (0 = clean)"
          exit 0   # TODO: change to `exit $fail` after one-time reconciliation
```

Naming the `exit 0` is not the whole finding, and stopping there would repeat the mistake. **The
job could never have been switched on, because it compared the wrong thing.** It ran `diff -u`
between the committed `.did` and the extractor output — and those two files differ enormously for
reasons that are not drift: hundreds of lines of hand-written documentation the extractor does not
reproduce (and which ship on-chain), plus `Result_N` renumbering and field reordering. Enabling a
byte diff would have demanded deleting the documentation. So it was muted, and being muted it saw
nothing. **A guard too loud to enable is a guard that is off.**

Replaced with `scripts/check-candid.sh` + `scripts/candid_compare.py`, which compare fully-resolved
method signatures: immune to comments, field order, alias names and `blob`/`vec nat8` sugar, and
unable to miss a dropped field, a changed type, an added/removed method or an update-call demoted
to a query.

The gate **proves it can go red before it judges**, on every run: it asserts it does *not* flag two
identical-but-differently-written interfaces, and that it *does* catch six planted defects.
Verified end-to-end by replanting each historical case in an isolated copy:

```
RED (exit 1)  Player.sitting_out_since removed   -> get_table_state ret0.Ok.players[]?.sitting_out_since ABSENT
RED (exit 1)  ActionRecord.phase removed         -> get_hand_history ret0?.actions[].phase ABSENT
RED (exit 1)  get_deposit_replay_state removed   -> METHOD IMPLEMENTED BUT NOT DECLARED
RED (exit 1)  lobby currency re-optionalised     -> get_available_tables ret0[].currency
```

`candid-extractor` is now pinned (`--version 0.1.6`); it was `cargo install candid-extractor
--locked` with no version, so an upstream release could have moved the gate under us.

<a id="d-10"></a>
### D-10 — high — the mainnet deploy installed an UNPINNED ic-wasm, in a step called "Install pinned toolchain" — STATUS: FIXED (wave 11)

**Status: found while enumerating the drift class, fixed 2026-08-06.**
`.github/workflows/deploy-ic.yml` ran:

```yaml
      - name: Install pinned toolchain (icp-cli + ic-wasm)
        run: |
          npm i -g @icp-sdk/icp-cli@${ICP_CLI_VERSION}   # ICP_CLI_VERSION: '1.0.0'
          cargo install ic-wasm --locked                 # <-- no version
```

The step's **name asserts a property its body does not have**. `ci.yml` proves the build
byte-reproducible with ic-wasm `0.9.9`; the mainnet deploy used whatever crates.io served that
morning. The `Dockerfile` states the consequence itself: *"a different ic-wasm shrinks differently
and the module hash moves."*

This attacks the only claim that makes this codebase checkable — that the deployed module hash
equals a reproducible build of `main`. A deploy on a day crates.io shipped a new ic-wasm would
produce modules matching no build anyone could reproduce, and the symptom is an auditor who cannot
match the hash. **That has already happened once here** (the wave that produced
`recipes/rust-reproducible.hbs`).

Also fixed: `deploy-ic.yml` and `cycles-monitor.yml` pinned icp-cli `1.0.0` while `ci.yml` and the
`Dockerfile` pinned `1.0.2` — and `icp.yaml`'s own header records that the wrong CLI/recipe pairing
makes *every* `icp` command in this project fail.

Gated by `scripts/check-declarations.sh` (CI job *Declarations agree with what they describe*),
which compares every copy of every pinned version and **fails on any unpinned install** of a tool
that moves the module hash. Both failure modes verified red on planted drift.

<a id="d-11"></a>
### D-11 — high — the frontend's Candid bindings are a third copy of the interface, and the fund-safety instruments are missing from it — STATUS: FIXED (wave 14)

> **FIXED 2026-08-09, and the drift was larger than this entry recorded.** Measured before the fix
> by comparing method sets:
>
> ```text
> table_1  canister .did 75 methods, .did.js 61
>          MISSING 16: admin_audit_deposit_custody, admin_get_deposit_custody,
>          admin_return_all_chips_to_escrow, claim_external_deposit, get_all_ledger_intents,
>          get_deposit_custody, get_deposit_replay_state, get_deposit_subaccount,
>          get_my_ledger_intents, get_solvency, refresh_deposit_custody,
>          refresh_main_account_custody, refresh_solvency, refund_external_deposit,
>          resolve_ledger_intent, resolve_my_ledger_intents
>          EXTRA 2: admin_restore_balance, deposit_from_external  <- the canister has neither
> history  18 vs 12, MISSING 6
> lobby    26 vs 24
> ```
>
> `claim_external_deposit` is the sweep an external wallet's deposit needs. `get_solvency` and
> `refresh_solvency` are [FINDING 35](SECURITY-FINDINGS.md#finding-35)'s whole instrument. The two
> EXTRA entries are worse than missing: a client that calls them fails at the wire.
>
> **The fix is a generator, not an edit.** `tools/gen-declarations` (detached crate, `candid_parser`
> — the same library `didc bind` uses) emits `<n>.did.js` and `<n>.did.d.ts` from the committed
> `.did`, and `scripts/check-declarations-js.sh` regenerates into a temp directory and diffs. A
> hand-written comparison can be wrong about what it compares (see [H-55](#h-55), found the same
> day, where exactly that had happened); *regenerate and diff* has nothing to be wrong about.
>
> **RED on the unfixed tree: 10 of 10 files.** Green after `--write`. The gate `--selftest`s by
> deleting `get_tables` from a binding and requiring a conviction, and it is in the CI job *Candid
> interface drift* and in step [1/9] of `dev.sh test`.
>
> The app was rebuilt, redeployed to the local asset canister and re-opened in a plain browser
> afterwards: the lobby still renders three tables from the regenerated bindings.

**Status: measured 2026-08-06, not fixed here — another owner's files.**
`src/cleardeck_frontend/src/lib/canisters.js` builds every actor from
`src/declarations/<n>/<n>.did.js`. That file is generated from the canister `.did` and committed,
and it has drifted independently of both.

Measured against the corrected source interface, `table_1.did.js` **does not declare**
`get_solvency`, `get_all_ledger_intents`, `get_cycle_status`, `refresh_solvency` or
`get_deposit_replay_state`. The custody and solvency instruments built in waves 7–10 —
[FINDING 29](SECURITY-FINDINGS.md#finding-29), [FINDING 35](SECURITY-FINDINGS.md#finding-35) —
are **not reachable from the UI at all**, because nobody regenerated the bindings.
`lobby.did.js` still declares `currency : IDL.Opt(Currency)` against a canister exporting a bare
`Currency`.

Both are masked by defensive frontend code — `utils.js:currencyOf` unwraps either shape,
`WithdrawModal.svelte:331` guards with `if (!tableActor?.get_custody_status)` — which is exactly
why nobody noticed. Note the third inconsistency: within `src/declarations/table_1/`, the `.did`
and the `.did.js` **disagree with each other** (`.did.js` has `hand_is_unmovable` and
`my_committed_in_pot`; the `.did` beside it does not).

The `.did` half is now baseline-guarded (`scripts/candid-declarations-baseline.txt`, 82 pinned
items, ratcheting down only). The `.did.js` half is unguarded — see
[DECLARED-VS-STORED.md](DECLARED-VS-STORED.md) row D4 for why, and for the fix: regenerate the
declarations from the corrected `.did` files. Owner: whoever owns `src/declarations` and
`src/cleardeck_frontend`.

<a id="d-12"></a>
### D-12 — medium — nothing asked whether mainnet still matched main; four defects of that shape have already landed — STATUS: FIXED (wave 11)

**Status: enumerated and scheduled 2026-08-06.** CI only ever proved things about the tree, never
about the fleet, so every claim about the running system decayed silently. Four defects share that
shape ([D-08](#d-08), the `icp.yaml` init-args case, the `Dockerfile` COPY list, the `.gitignore`
build input) and the sharpest of them — `btc_table_1` running at one tenth its declared stakes for
its whole life — was found **by a player**, not by us.

The full inventory of every place this repository declares something stored or built elsewhere,
with the guard status of each row, is now **[docs/DECLARED-VS-STORED.md](DECLARED-VS-STORED.md)**
(24 rows across interface, config/deployment, toolchain, build inputs and the registers).

`.github/workflows/deployed-drift.yml` runs `scripts/check-deployed.sh` daily. It compares, for
every mainnet canister: the revision it reports, whether it was built from a clean tree, whether
the whole fleet is on ONE revision, the `candid:service` interface it publishes against the
committed `.did`, and the live `TableConfig` against `icp.yaml` (via the existing
`check-deployed-config.sh`). It opens or updates a `deployed-drift` issue on failure.

**It is read-only and cannot deploy.** Its only network operations are `icp canister metadata` on
public sections and `icp canister call --query`; it needs no identity and no secrets. A first step
greps itself and the scripts it calls for any mutating `icp` command and fails the run before the
network is touched — and self-tests that the pattern matches a real mutating command, so the guard
cannot be vacuous. Both verified locally.

Exercised end-to-end against the local replica, where it correctly passed `history`, correctly
reported the stale interface on `lobby` and the tables, and correctly passed all four table
configs. **Not yet exercised against mainnet:** this wave was forbidden from calling it. The first
scheduled run is the real proof, and it is expected to report drift until the lead redeploys —
see the hash change in [D-08](#d-08).

<a id="e-84"></a>
### E-84 — fund-theft — one key on the controller seat erases every balance, and no in-canister check can reach it — STATUS: OPEN

**This is [FINDING 23](SECURITY-FINDINGS.md#finding-23) with the id it never had.** For four waves
that finding was marked `—` in the DEFECTS.md column and "scheduled from THE FINDINGS", and it sat
unscheduled from wave 7 to wave 12 while being the worst thing in either file. An entry nothing can
point at is an entry nothing picks up.

**The defect.** Every ClearDeck canister has one controller principal. A controller can call
`install_code --mode reinstall` or `uninstall_code` on a funded table. Measured on the local
replica against the real table wasm and the real ICP ledger
(`tests/money_safety/tests/controller_custody.rs`, `./scripts/dev.sh custody`):

```text
BEFORE   ledger_at_canister=4000000000  escrow=2600000000  chips=1400000000
         alice escrow=1300000000  bob escrow=1300000000
reinstall_canister(controller) -> Ok
AFTER    ledger_at_canister=4000000000  (unchanged: the ICP is still there)
         alice escrow=0   withdraw(5 ICP) -> Err("Insufficient balance. Have: 0.0000 ICP, ...")
DESTROYED 4000000000 e8s = 40.00000000 ICP of player claims, with the ledger untouched
```

Same wasm, no code change. `uninstall_code` is worse: the canister has no code left, so there is
no surface to ask.

**Why every instrument in this project was silent, structurally.** `admin_custody.rs` is the gate
on the controller surface and its subject is methods that call `require_controller()` INSIDE the
canister. These two are calls to the MANAGEMENT canister. The table never sees them, cannot refuse
them, and no in-canister audit surface can be extended to cover them. That is why the control has
to be the CONTROLLER LIST itself.

**What exists now.** `src/guardian_canister/` (a controller-of-controllers), built by
`./scripts/build-guardian.sh`, gated by `./scripts/dev.sh custody` (9 tests, in `dev.sh test`).
The premise it rests on — that controllership is not transitive — is measured in both directions
before anything is built on it. See FINDING 23 for what it does and does not prevent.

**What is still OPEN, and it is the whole entry.** The guardian is **not deployed** and holds the
controller seat on nothing. Mainnet is exactly as described above. Closing this needs, in order:
deploy the guardian to mainnet; `update_settings` each fund-holding canister to
`controllers = [guardian]`; `update_settings` the guardian to `controllers = [guardian]`; set a
long freezing threshold on all of them FIRST, because after the handover nobody can change their
settings ever again. None of that is this wave's to do — it is a mainnet operation and this tree is
forbidden from touching mainnet.

<a id="d-13"></a>
### D-13 — high — "fully decentralized" over a canister one key can empty — STATUS: FIXED (wave 12)

Nine waves went into deleting claims this software could not back. This was the largest one left,
and it was in the first paragraph:

> ClearDeck is a **fully decentralized** Texas Hold'em poker application running entirely on the
> Internet Computer. Every card shuffle is cryptographically verifiable, ensuring **fair play
> without requiring trust**.

Both halves of the second sentence are true of the SHUFFLE and neither is true of the CUSTODY. The
README's only disclosure of controller power was about the hand history —

> a controller of the archive canister can still destroy every proof by reinstalling or deleting
> it. No application code can prevent that

— which is the same call, one canister over, doing something far worse and not saying so. The
deposit screen, the one place a player is about to move real money, said nothing about it at all.

**Fixed by addition, never by softening.** The four protected notices and the no-rake property are
untouched and still measure 5/5 unobstructed at 1440x900 and 390x844 with the deposit modal open,
re-measured with `tools/shots/lib/protected-notices.mjs` after the change. Added:

* `README.md` — the first paragraph now says it is not trustless and not fully decentralized, and
  points at a new **Who can take your money** section carrying the measured transcript, a
  can/cannot table, and an explicit list of what the guardian would and would not prevent;
* `Security Considerations` item 4 is now the controller seat rather than "secure the controller
  identity", and item 3 names cycle exhaustion as the same destruction by another route;
* `Known Issues` says the call that destroys proofs destroys BALANCES;
* `DepositModal.svelte` — an amber `custody-notice`, after the four protected notices and before
  every control that can move money, measured on rendered pixels at both viewports.

**Gate:** `./scripts/dev.sh hygiene` step *custody disclosure present (FINDING 23)*: seven required
phrases across README.md and the frontend, plus a check that the `fully decentralized` claim has
not come back (written so the DENIAL does not trip it). Every one of the seven was negative-tested
by removing it and confirming hygiene goes red; `## Who can take your money` carries its heading
markers because without them the check was satisfied by the two cross-references further down the
file, so deleting the section and leaving dangling links read green.

**This is a disclosure, not a fix.** [E-84](#e-84) is open and mainnet is unchanged.

<a id="h-48"></a>
### H-48 — high — one of the four unrun gates is RED, and it is M9's own file — STATUS: FIXED (wave 14)

> **SETTLED 2026-08-09 by running it, and by answering the question the entry actually asked.**
>
> ```text
> $ cd tests/money_safety && CLEARDECK_TABLE_WASM=… cargo test --test fund_reachability -- --test-threads=1
> test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.04s
> ```
>
> against wasm `3890a6d4a1861343df40c08772979171bc15fed8c51102c81f3c849346b29a0d`. Not somebody's
> report: this run.
>
> **Which of the two readings was right: the GATE was stale, the code was right.** `git log -p` on
> the file shows both assertions were rewritten in commit `134550e`, from a single
> `world.advance(30 + 300 + 5)` to `for _ in 0..12 { world.advance(60) }`, with the reason written
> into the file:
>
> > *Since E-59 a hand is stuck when the canister has WATCHED it fail to move — committed messages
> > spanning `STUCK_HAND_GRACE_NS` — and not when a wall clock says so. A single `advance(335s)`
> > gives the canister nothing to witness, so it correctly refuses, and a test that jumped once was
> > measuring the old semantics.*
>
> and the resulting coverage loss recorded rather than hidden: the second test no longer exercises
> the `hand_is_stuck` branch inside `withdraw`/`cash_out` at all, and its doc comment says so and
> names what still does. That is the judgement this entry was waiting for.
>
> It is run by `dev.sh test` step [5/9] and by the `fast` CI tier, and it carries
> `m9s_per_step_check_can_actually_fail`, so it is not a gate that cannot go red.

> **WAVE 12: IT IS GREEN NOW, 6 of 6, on the wasm `dev.sh test` builds** — and it is still `OPEN`,
> because "green today" is not the claim this entry makes. Re-run 2026-08-07 through
> `dev.sh test`'s own money-safety block:
>
> ```text
> fund_reachability   test result: ok. 6 passed; 0 failed  (12.57s)
>   m9_a_hand_nobody_can_move_is_abandonable_by_anybody
>   m9_a_silent_client_mid_hand_does_not_lock_the_table
>   m9_the_auditors_disconnect_sequence_leaves_every_door_open
>   m9_the_in_a_hand_refusal_lifts_once_the_hand_cannot_progress
>   m9s_per_step_check_can_actually_fail
>   no_trapping_evaluator_is_reachable_from_an_update_entry_point
> ```
>
> The two that were red were at the predicate [E-59](#e-59) changed from the wall clock to
> ATTEMPTS, which is exactly the "probably a stale gate" this entry guessed. The reason it stays
> OPEN is that **nobody has established which of the two readings was right**: a gate that went
> green when the code around it changed, with no one deciding whether the assertion or the code
> moved, is the same unfinished state it was in when it was red. It is now RUN by a named target
> ([H-45](#h-45) closed that half), so the next wave inherits a fact instead of a question.

**Status: executed 2026-08-06.** Running the four [H-45](#h-45) targets by hand against the wasm
`./scripts/dev.sh test` had just built (`sha256 c05fbdc6…`):

```
stall_agreement             2 passed  0 failed      (M13 ONE BELIEF, 803 s)
solvency                   12 passed  0 failed
deposit_subaccount_anchor  11 passed  0 failed
fund_reachability           4 passed  2 FAILED      <-- M9 FUND REACHABILITY
```

**Three of the four are green, which is the good news and also the point: nobody knew.** The
fourth is red:

```
m9_a_hand_nobody_can_move_is_abandonable_by_anybody
  tests/fund_reachability.rs:266  "an hour-dead clock is a stuck hand"

m9_the_in_a_hand_refusal_lifts_once_the_hand_cannot_progress
  tests/fund_reachability.rs:349  escrow must be withdrawable once the hand cannot progress:
  Err("Cannot withdraw while in a hand. 0.0200 ICP (2000000 e8s) of yours is committed to hand 1
       … A hand only becomes refundable once this canister has watched it fail to move for 5
       minutes …")
```

Both failures are at the predicate [E-59](#e-59) / [FINDING 25](SECURITY-FINDINGS.md#finding-25)
changed. `hand_is_stuck` used to be the wall clock; it is now about **attempts** — the canister
must have watched the resolution path fail across 3 committed messages *and* 300 s. Both tests
reach 335 s with a single `World::advance`, which is `advance_time` plus **one** `tick`, so the
canister gets one observation where the new predicate wants three.

**The most likely reading is a stale gate, not a fund lock**, and there is real evidence for it:
`timers::the_table_settles_itself_with_no_external_caller` is in `./scripts/dev.sh test`, is
green, and proves the table settles itself with **zero ingress messages** in 30 s and releases
every seat at 210 s — so on a subnet, where rounds keep happening, the money is reachable. That is
a reading, not a measurement, and this entry does not claim more.

**Why it is filed high anyway.** M9 is the property this project added after an auditor found a
funded table with ~420 ICP unreachable ([FINDING 15](SECURITY-FINDINGS.md#finding-15)). Its
dedicated file has been red for some part of the last two waves and **no target runs it, so no
wave had to answer for it.** A red gate nobody runs is worse than a missing one: it will be found
eventually by someone who cannot tell whether it means the code is broken or the test is old, and
they will be right not to know. Either the tests are updated to the E-59 predicate (tick, don't
jump) in the same change that wires them in ([H-45](#h-45)), or the predicate is wrong and this is
E-59's residue.

Not fixed here: `tests/money_safety/**` is another owner's tree, and the fix is a judgement about
which of the two is correct, not an edit.

---

## Found by the wave-11 oldest-cluster pass

The six oldest entries in the findings register ([FINDING 02](SECURITY-FINDINGS.md#finding-02),
05, 08, 09, 17, 22) re-asked on the current tree. Five were already dead and said otherwise; the
sixth was real. Two more defects fell out of proving it, and one of them is in the instrument.

<a id="e-78"></a>
### E-78 — high — the recovery door could void a live hand, and left a zombie hand behind — STATUS: FIXED (wave 11)

**Status** **REPRODUCED on module `792a9487…` and FIXED 2026-08-06.**
[SECURITY-FINDINGS.md FINDING 22](SECURITY-FINDINGS.md#finding-22) carries the full write-up,
the trade-off table and the decision; this entry is the register's copy.

**Where** `return_all_table_custody_to_escrow`, reached by `admin_return_all_chips_to_escrow` and
`admin_reinit_table`, `src/table_canister/src/lib.rs`.

**What was wrong** Two things of different sizes, in one function.

1. **The design question.** The door worked on a hand that was being actively played. A
   controller can read every hole card through `get_table_state`, so they could read the cards
   and then decide whether the hand happened. It conserves to the e8 — every wager back to the
   player who made it — so **not one invariant in this project could see it**: what is taken is
   not principal but the equity a player has already bought.
2. **The bug.** Whatever it did to the money, it left the hand OPEN: `phase`, `action_on` and
   every hole card untouched, every stack at zero, the table sitting mid-street. `reload` then
   refused *"Cannot reload during a hand"* until the zombie hand finished.

```
controller sees: phase=Flop pot=6000000 cards=[(0, Jd/Ac), (1, Th/5d), (2, 6d/3h)]
admin_return_all_chips_to_escrow -> Ok
after: phase=Flop pot=0 action_on=2 cards_still_dealt=3   every player +2000000 (their own stake)
invariant violations: 0
hand 1 in the permanent record: winners: [], community_cards: [], participants: None
```

**The fix** The power is KEPT and narrowed, because closing it on a hand the canister cannot
prove is dead would leave a trapping canister with no privileged escape at all, which is
[FINDING 15](SECURITY-FINDINGS.md#finding-15).

* The door refuses while `hand_in_progress(state) && !hand_cannot_move_right_now(state, now)`.
  That predicate is pure state (`now > action_timer.expires_at`, or no timer), so no trap, no
  lost timer and no missing stall witness can stop it becoming true — which is why it, and not
  `hand_is_stuck`, is the right gate. Cost to an honest operator: one action timeout.
* A live hand it does reach is closed through `settle_unmovable_hand`, the same routine the
  permissionless `abandon_stuck_hand` uses, so the controller does what any principal could.
* Every credit of that hand is archived with `pot_type = "refund:ended-by-controller"` — an
  existing field, so **no Candid type changed** (`./scripts/check-candid.sh`: 0 structural
  differences) — and the canister logs it at `CRITICAL:`, which the money-safety classifier
  treats as run-stopping.

**Gates** `oldest_cluster::finding22_the_recovery_door_refuses_a_hand_that_can_still_be_played`
and `finding22_a_hand_a_controller_does_end_is_closed_and_permanently_marked`. Also
`admin_custody::admin_reinit_table_mid_hand_returns_the_pot_to_the_players_who_put_it_in`, which
**used to pin this defect** (`assert!(outcome.is_ok(), "admin_reinit_table mid-hand must
succeed")`) and now drives both halves.

<a id="e-79"></a>
### E-79 — medium — every hand refunded rather than won read back as a blank record — STATUS: FIXED (wave 11)

**Status** **FOUND AND FIXED 2026-08-06**, while proving E-78's permanent record.

**Where** `settle_unmovable_hand` vs `settle_hand`, `src/table_canister/src/lib.rs`.

**What was wrong** The write into the local 100-hand ring (`HAND_HISTORY`) and the "last hand"
display slot lived **inline in `settle_hand`**. `settle_unmovable_hand` does not call
`settle_hand` — it builds its own refund plan and applies it — so it never reached that write. So
every hand ever closed by `abandon_stuck_hand`, by the on-chain clock, by an exit door finding
the hand unmovable, or (now) by the recovery door read back from `get_hand_history` as:

```
HandHistoryAmounts { hand_number: 1, winners: [], community_cards: [], showdown_players: [] }
```

A blank record of a hand in which real money moved back to real people — while the **archive
canister had every credit**, because `record_hand_to_history` was called on both paths. The two
records disagreed about every refunded hand, which is the same shape as
[FINDING 30](SECURITY-FINDINGS.md#finding-30): the local copy and the permanent copy deriving the
same fact separately and drifting.

**The fix** One function, `record_local_hand_result`, with two callers. Not a copied block.

**Gate** `oldest_cluster::finding22_a_hand_a_controller_does_end_is_closed_and_permanently_marked`
asserts the recorded credits sum to exactly what the hand collected, which is 0 against a blank
record.

<a id="h-49"></a>
### H-49 — high — the settlement oracle had never once executed `cash_out` or `check_timeouts` — STATUS: FIXED (wave 11)

**Status** **FOUND AND FIXED 2026-08-06.** This is worth more than the fix it was found under.

**Where** `tests/settlement/src/suite.rs` and `src/drive.rs`.

**What was wrong** Every scenario that vacates a seat is written as *"call `leave_table`, and
fall back to `cash_out` if that fails"*:

```rust
if w.leave_table(who).is_err() {
    let _ = w.cash_out(who);
}
```

`leave_table` never fails for a seated player. So across **53 compared hands in four benches**,
`cash_out` never ran inside the settlement oracle, and neither did `check_timeouts` — and the
oracle is the only instrument in this project that asks *who was PAID* rather than *do the totals
balance*.

That matters because [FINDING 08](SECURITY-FINDINGS.md#finding-08)'s whole point is that the
orphaned-stake state is reachable by a **disconnect**: a player closes their tab, their own action
clock folds them, and they can then vacate the chair with money in the pot. The gates that do
cover that route (`reg08`, M1b) ask whether the pot is still fully **attributed**. Attribution and
payment are different questions and they come apart exactly where
[FINDING 13](SECURITY-FINDINGS.md#finding-13) lives — right amount, right chair, wrong person.

**The fix** `suite::timed_out_seat_cashes_out_mid_hand`, in `run_all`, plus a dedicated
`a_seat_folded_by_its_own_clock_and_cashed_out_mid_hand_settles_by_the_rules`. `cash_out` is the
**only** door it calls, so if that door ever stops opening the fixture assertion goes red rather
than the coverage quietly going away. It runs over a short all-in so the departing stake is money
that can move between layers, and it requires the short stack to hold the best hand, or a deep
seat wins every layer and the move is invisible in the payout.

```
--- seat_timed_out_then_cashed_out_mid_hand | hand #1 | button seat 1 | 6 seats
seat  hole    contributed  folded  left   engine_delta  oracle_delta  DIFF
   0  3s 9c            20      no    no            60            60      +0
   2  Ad 5s            60     yes   yes           -60           -60      +0   <- clock folded it, then cash_out
oracle pot layers:
  layer 0 (0..20]  amount 80   eligible [0, 1, 3]  winners [0]
  layer 1 (20..60] amount 120  eligible [1, 3]     winners [3]
collected 200   oracle owed 200   engine paid 200   destroyed 0
```

---

<a id="e-80"></a>
### E-80 — medium — `notify_deposit` asserts a negative it cannot know, and points at the wrong address to check it against — STATUS: OPEN

**Status** OPEN. Found by the wave-11 critic and re-confirmed in the wave-11 reconciliation.

`src/table_canister/src/lib.rs`, the destination branch of `notify_deposit`:

```rust
if to_bytes != expected_to {                    // expected_to is the MAIN account
    let own_deposit = compute_account_identifier(&canister, Some(compute_deposit_subaccount(&caller)));
    if to_bytes == own_deposit { /* correct, helpful */ }
    return Err("Transfer was not to an account of this canister. ... \
                Check the destination against get_deposit_address().")
}
```

Two things are wrong with the fallback, and they arrived from two different agents in the same
wave:

1. **The claim is false in the case that matters most.** Any destination that is neither the
   main account nor the caller's own deposit address takes this branch — including *another
   player's* deposit subaccount, which is an account of this canister, and is precisely where
   [FINDING 40](SECURITY-FINDINGS.md#finding-40)'s substituted address puts the money. The
   canister cannot invert the hash to name the owner, and it does not need to in order to stop
   asserting something it cannot know.
2. **The instruction points at the wrong address.** `get_deposit_address()` used to return the
   main account; since [FINDING 34](SECURITY-FINDINGS.md#finding-34) it returns the caller's
   own. This sentence is about the main account and now tells the reader to compare against a
   different one.

**What would close it:** say what is known ("this transfer did not go to the main account, and
it did not go to your deposit address") without asserting what is not, and name the main account
explicitly rather than by a method that no longer returns it.

---

<a id="e-81"></a>
### E-81 — medium — the withdrawal refusal states a universal guarantee that the same sentence disproves — STATUS: FIXED (wave 14)

**Status** FIXED in wave 14, alongside [E-89](#e-89) — it is the sentence the player stranded by
[FINDING 31](SECURITY-FINDINGS.md#finding-31) reads, so closing one without the other leaves the victim
of the fixed defect still being told to retry. Raised by the fifth auditor.

> **THE FIX: TWO REFUSALS, BECAUSE THEY ARE TWO SITUATIONS.** A policy floor is something a caller can
> clear by asking for more. The ledger's own fee is not. They read the same and only one of them can be
> acted on, so `withdraw` now branches on `balance_now <= transfer_fee` and answers:
> *"No withdrawal of any size can move this, and that is arithmetic rather than a policy of this table…
> Not by you, not by this table, and not by a controller: there is deliberately no method here that can
> edit a balance. IT IS NOT LOST AND IT IS NOT FORGOTTEN — it is counted in get_custody_status() and in
> get_solvency(), and if you ever put more into this table it comes out with the rest in a single call."*
> The policy sentence is unchanged for every balance it is actually true of.
>
> **AND THE SAME SENTENCE WAS IN THE UI, WHICH IS THE SURFACE THE PLAYER ACTUALLY READS.**
> `WithdrawModal.svelte` mirrors the canister's floor client-side, deliberately, so a
> client-side floor cannot become a trap the canister's gates cannot see -- and it mirrored
> the wrong half: *"…can always be withdrawn in one call whatever its size … **press MAX**"*,
> to a player whose MAX will be refused. It now branches on the same condition and says the
> same thing the canister says. `ui_limits` stays green (11/11): every figure is still
> interpolated from the mirrored constants and no literal came back.
>
> **The gate asserts the two absences and the two presences**, because the defect was what the sentence
> SAID and not whether it errored: no `"can always be withdrawn"`, no `"Minimum withdrawal is"`, and it
> must name the `network fee` and say `No withdrawal of any size`. Verified red against HEAD, which
> answers the paragraph quoted below verbatim.

`src/table_canister/src/lib.rs`:

```
Minimum withdrawal is 0.0002 ICP. Your whole remaining balance can always be withdrawn in
one call whatever its size, as long as it is more than the 0.0001 ICP network fee -- you
have 0.0001 ICP.
```

The `always ... whatever its size` clause is false, and the qualifier that follows is what makes
it false. This is the exact message shown to the player whose deposit is stranded by
[FINDING 31](SECURITY-FINDINGS.md#finding-31): they read it, conclude they made a formatting
mistake, and retry. There is no amount and no later that gets it out.

**What would close it:** state the reachable bound for THIS balance, and when the balance is at
or below the fee, say plainly that no amount can move it and why.

---

<a id="e-82"></a>
### E-82 — medium — the replay refusal claims the money is in your balance, on a canister where it is not — STATUS: OPEN

**Status** OPEN. Raised by the fifth auditor, in the state
[FINDING 23](SECURITY-FINDINGS.md#finding-23) produces.

`notify_deposit`'s ICRC-2 branch answers:

> *"This block is an ICRC-2 pull performed by this canister on your behalf (the deposit() flow).
> It was credited to your balance when the pull happened and cannot be credited again."*

On a canister whose stable state has been wiped by a controller `reinstall`, the second sentence
is false: the balance is 0. The refusal itself is correct — it is replay protection and must not
weaken — but the reason it states is a claim about the books, and the canister can read the
books before making it.

**Why it matters:** it is the last door a wiped-out player tries, and it tells them their money
is already safely in their balance. That converts a recoverable support case into a user who
stops looking.

---

<a id="e-83"></a>
### E-83 — low — `buy_in` on your own seat answers "Seat is taken" — STATUS: OPEN

**Status** OPEN. Raised by the fifth auditor.

`join_table(seat)` silently auto-buys-in at `min_buy_in`. A player who then calls
`buy_in(seat, amount)` on the seat they are sitting in gets `"Seat is taken"`
(`src/table_canister/src/lib.rs:4789`). The seat is taken by the caller. Topping up requires
`reload`, which the error does not mention.

No money is lost. It is the first interaction at the table and it lies about who is sitting
there, and a player who believes it may go looking for another table.

---

<a id="t-41"></a>
### T-41 — critical — the mainnet deploy workflow could not complete, in the wave that made a redeploy mandatory — STATUS: FIXED (wave 11)

**Status** **FOUND AND FIXED 2026-08-06**, in the wave-11 reconciliation. Raised by the
wave-11 candid critic; reproduced here with the workflow's exact environment.

### What was wrong

`.github/workflows/deploy-ic.yml`, "Build frontend":

```yaml
env:
  VITE_CANISTER_ID_LOBBY: kpfcd-kyaaa-aaaaj-qor3a-cai
  VITE_CANISTER_ID_HISTORY: kggj7-4qaaa-aaaaj-qor2q-cai
run: |
  npm ci
  npm --workspace src/cleardeck_frontend run build      # <-- no DFX_NETWORK anywhere
```

[T-01](#t-01)'s wave-9 fix makes any build that does not STATE its target a hard error, because
a bare `npm run build` used to fall back to the repo-root `.env` holding the MAINNET ids.
Reproduced verbatim:

```
$ env -u DFX_NETWORK VITE_CANISTER_ID_LOBBY=... VITE_CANISTER_ID_HISTORY=... \
    npm --workspace src/cleardeck_frontend run build
...
  local       DFX_NETWORK=local ... npm run build
  mainnet     npm run build:mainnet
Why this is mandatory: the repo-root .env holds the MAINNET canister ids, which
custody real ICP and ckBTC. ... See docs/DEFECTS.md T-01.
npm error Lifecycle script `build` failed with error: code 1
```

That step precedes the snapshot step and the backend deploy, so **nothing was deployed at all**
— not the frontend, not the six backend canisters.

### Why nobody saw it

`.github/workflows/ci.yml`'s frontend job sets `DFX_NETWORK: ic` on its build. So CI proved the
build works, in an environment the deploy does not use. That is this project's signature failure
shape in the build system rather than in the money: **a green gate measuring something adjacent
to the thing that has to work.**

### Why it was a merge blocker this wave and not just a latent one

[FINDING 03](SECURITY-FINDINGS.md#finding-03) corrected the committed `.did` files, and the
`.did` is a build input (`recipes/rust-reproducible.hbs` embeds it as the module's on-chain
`candid:service` metadata). Every module hash therefore moved, and mainnet does not match this
tree until the lead redeploys — through a workflow that could not run.

### The fix

`npm run build:mainnet`, which is the named path T-01's own fix file points at: it states the
target in the command, refuses a contradictory ambient `DFX_NETWORK`, and **verifies the built
bundle before exiting 0**, deleting the dist if verification fails so `icp deploy` cannot upload
an unverified one. Run locally with the workflow's exact environment: **13 of 13 checks passed,
READY TO DEPLOY.**

This also makes a register entry true rather than aspirational: [T-01](#t-01) and
[T-38](#t-38) both name `npm run build:mainnet` / `npm run verify:deployed` as their gate, and
until now **nothing in this repository invoked either**.

---

<a id="d-14"></a>
### D-14 — fund-theft — the custody disclosure understated THEFT as destruction, on the deposit screen — STATUS: FIXED (wave 12)

`D-13` closed a nine-wave silence by putting a custody disclosure on the deposit modal and a
`Who can take your money` section in the README. One clause of it was **false**, and false in
the direction that flatters the operator:

> "Your ICP would stay at the canister's ledger address, unreachable by anybody — including the
> operator, who [could not pay] it to themselves. [Destruction, not] theft, and permanent either
> way."

and, in the README's decision table:

> | Can the operator take the ICP out of the canister to their own wallet? | **No.** There is no
> method that pays a controller … |

**Both are wrong.** "There is no method that pays a controller" is a true statement about
ClearDeck's code and an irrelevant one about a controller, because **a controller replaces the
code.** `install_code` installs whatever module it is handed, and the canister's ledger account
is spendable by whatever is then running in it. The money never had to pass through a ClearDeck
method.

### Executed, with the same verb as the wipe

`tests/money_safety/tests/controller_custody.rs`,
`finding23c_the_operator_can_pay_the_ledger_balance_to_themselves`, run by
`./scripts/dev.sh custody` and by `./scripts/dev.sh test`:

```text
--- FINDING 23c: the operator does not have to destroy it. They can TAKE it ---
BEFORE   ledger_at_canister=4000000000  escrow=2600000000  chips=1400000000
         operator_wallet=1000000000000
thief module sha256=4c3be308… (508924 bytes, NOT ClearDeck)
reinstall_canister(controller, thief_module) -> Ok
steal(3999990000) -> icrc1_transfer accepted
AFTER    ledger_at_canister=0  operator_wallet=1003999990000
MOVED    3999990000 e8s = 39.99990000 ICP of player deposits into a wallet the operator owns
```

`tests/thief_canister` is that module: init with a ledger and a payee, one `steal`, one
`icrc1_transfer`. It is a detached crate under `tests/`, so it touches neither the root
`Cargo.lock` nor `icp.yaml` and no deploy path can reach it.

### Why the existing gate was green over it

`cmd_hygiene`'s custody check is a **presence** check: seven phrases that must appear. Every one
of them was present. The false sentence was *additional* reassurance sitting next to true ones,
which no presence check can see. The retraction therefore needed an **inverted** gate of its own
— `RETRACTED_CUSTODY_CLAIMS` in `scripts/dev.sh`, the same shape as the `fully decentralized`
check — asserting that the two retracted sentences are **absent** from `README.md` and the
frontend. Case-sensitive, so this file and
[FINDING 23](SECURITY-FINDINGS.md#finding-23) can still discuss the retraction in prose.

**The general lesson, and it is the reason this is filed at `fund-theft` rather than as a copy
defect:** a disclosure that is wrong in the operator's favour is worse than no disclosure,
because a player who reads it deposits with a false floor under them.

---

<a id="e-85"></a>
### E-85 — medium — the Leave-table control renders outside a 1440 px window — STATUS: OPEN

`.sit-controls > button.control-btn.destructive` measures `51.2×28 at x=1396` on
`table-preflop` and `table-facing-bet` at 1440×900, so its right edge is at 1447.2 in a 1440 px
frame. `documentElement.scrollWidth` is 1440 and `<body>` carries `overflow-x: hidden`, so the
tail is simply not on screen and **cannot be scrolled to**.

Found by `tools/shots/lib/table-in-frame.mjs` and invisible to every other gate in the repo:
`occlusion.mjs` compares element rectangles **against each other**, never against the frame, so
a control that leaves the viewport entirely is not an occlusion.

The wrapping fix was attempted and reverted with the measurement recorded in the `.dock-aux`
comment in `PokerTable.svelte`: it puts the button inside the frame and costs 43 px of dock,
dropping the desktop felt from 27.8% to 23.2%. The structural fix is to move `.sit-controls`
into the left dock cell, which has ~435 px of unused width — a deliberate relocation of a
control, not a bug fix. [DESIGN-BAR §11.7](DESIGN-BAR.md).

---

<a id="e-86"></a>
### E-86 — high — the deposit gate compared four correct figures against three wrong chain quantities — STATUS: FIXED (wave 12)

**THE EIGHTH CROSS-AGENT DEFECT. Correct totals, wrong recipients, every invariant silent.**

`chain-agreement.mjs` read the deposit modal's `.minimum-notice`, took every number out of it
**in DOM order**, and assigned them positionally:

```js
nums[0] -> the minimum the table canister enforces
nums[1] -> the ledger's icrc1_fee()
nums[2] -> minimum + 2 x fee
```

At `134550e` the copy changed. The modal now names **four** quantities, in this order: the
minimum to the deposit ADDRESS (`MIN_WITHDRAWAL + FEE`, 30 000 e8s), the lower minimum from a
connected wallet (20 000), the ledger fee (10 000), and the wallet balance needed (40 000). So:

| screen figure | what it is | what it was compared with | verdict |
|---|---|---|---|
| 0.0003 | the address minimum | `min_deposit` = 20 000 e8s | **DISAGREES, 1.500x** |
| 0.0002 | the connected-wallet minimum | `icrc1_fee()` = 10 000 e8s | **DISAGREES, 2.000x** |
| 0.0001 | the ledger fee | minimum + 2 x fee = 40 000 e8s | **DISAGREES** |
| 0.0004 | the wallet requirement | *nothing* | census: unaccounted for |

Both deposit shots were filed `UNVERIFIED` with *"DEPOSIT CHAIN DISAGREEMENT … screen is 1.500x
the chain"*. **The screen was right. Every figure on it was correct and correctly computed. The
comparison had been silently re-pointed at the wrong quantity by a copy edit**, and the one new
figure — the largest of the four, and the one a player with a nearly-empty wallet acts on — was
asserted by nothing.

This is the same seam as [H-41](#h-41), which is the entry about the copy growing and the
assertion not; the difference is that H-41 lost a figure and this one **kept the count and lost
the subjects**, which is why it read as a product defect rather than as an instrument defect and
would have been "fixed" by changing the screen.

### The fix

The four claims are now anchored on their **labels** (`Minimum to this address:`,
`lower minimum of`, `network fee`, `you need`), each against the quantity it actually names, and
a label that stops matching is a **structural failure** — "the figure it names is now checked by
nothing" — rather than a silent re-pointing. `config.mjs`'s mirrored constant is joined by the
derived external minimum, computed from the same two numbers the canister derives it from.

---

<a id="e-87"></a>
### E-87 — high — the red ledger acknowledged a SHOT, not a PROBLEM — STATUS: FIXED (wave 12)

`artifacts/screens/acknowledged-reds.json` matched an entry to a red shot by
`(scene, viewport)`. Once a shot was red for **any** reason, every **later** failure on the same
shot joined the same `problems` array and was excused by an entry that had never been about it.

Not hypothetical. `table-in-frame.mjs` landed in wave 12 and its first real conviction —
[E-85](#e-85), a Leave-table control 6–7 px outside the frame, which a player cannot reach —
appeared on `table-preflop` and `table-facing-bet` at desktop. Both were already acknowledged
under [E-76](#e-76) for a felt-area shortfall of 0.2 points. `dev.sh shots-verdict` stayed green.

**Rule 3:** an entry must now list `covers` — the substrings of the recorded problems it
accounts for. Every recorded problem on an acknowledged shot must match one, and every `covers`
string must still match a recorded problem, so a cover that stops applying fails exactly the way
a stale acknowledgement does. A cover shorter than 12 characters is refused.

---

<a id="e-88"></a>
### E-88 — medium — the tracked verdict drops any failure phrased in words its vocabulary does not know — STATUS: FIXED (wave 12)

`verdicts.mjs` `problemsOf` splits a scene's notes into clauses and keeps the ones matching
`/FAILED|DISAGREE|RAKE TAKEN|STRUCTURAL|MISSING|COVERED/i`. **When at least one clause matches,
every other clause is discarded.**

Neither of the two geometry checks uses any of those six words. The felt floor emits *"below the
45% floor"*; `table-in-frame.mjs` emits *"is NOT fully inside the 1440x900 frame"*. They reached
`verdicts.json` only because they happened to be the **only** clause on their shots, so the
`clauses` fallback fired. The first shot to fail an occlusion check **and** the felt floor would
have recorded the occlusion and lost the felt out of the one file `dev.sh shots-verdict` reads —
a real red, deleted before the gate ever saw it, on a run that reported the correct number of
problems.

Fixed by adding every phrase the checks actually emit to the vocabulary, with a comment saying
that a new check must add its own.

---

<a id="h-50"></a>
### H-50 — high — the archive analyser's 39 gates were run by nothing — STATUS: FIXED (wave 12)

[H-45](#h-45)'s shape, in the wave that closed H-45. `tools/archive/**` shipped with 39 offline
self-tests and **no `dev.sh` target, no `make` rule and no CI job** — while
[docs/ARCHIVE.md §8](ARCHIVE.md) stated they held everything above it, in a section whose first
sentence is *"A gate nothing runs is worse than no gate."*

It matters more than an unrun test suite usually does. `tools/archive` is an **independent
reimplementation** of [SHUFFLE-SPEC](SHUFFLE-SPEC.md) that shares no line with `poker_core`, the
canister or `src/declarations`, so it is the only instrument in the tree that can catch those
being wrong **together**. A second opinion nobody runs is one opinion.

`./scripts/dev.sh archive` (and `make archive`) now runs it, and `dev.sh test` runs it as step
7 of 8. ~85 s, no replica, no network: every population it judges is built by the test.

---

<a id="h-51"></a>
### H-51 — high — the table-in-frame gate had no gate, and its own headline conviction was unprotected — STATUS: FIXED (wave 12)

`grep -rn table-in-frame tools scripts Makefile` returned exactly one hit — `tools/shots/run.mjs:41`
— so the only thing that ran the wave's best new instrument was the full sweep, which needs a
live replica and a browser. That is the precise condition
`artifacts/screens/acknowledged-reds.json` blames for [E-63](#e-63) sitting red for a whole wave.

**And the module could lose its own conviction silently.** `.seat` is `width: 0; height: 0` — a
point on the ring — so a box measured on it is inside the frame by construction and cannot fail.
The module measures the UNION of each seat's painted descendants for exactly that reason.
Restore the naive measurement and `foldTableInFrame` reported *"9/9 seat pods, all in frame"*
over zero-size boxes, because every check below it iterates a set that is now empty.

`tools/shots/test-table-in-frame.mjs` is the gate: 17 cases over `foldTableInFrame`, which is a
pure function and needs neither replica nor browser. Three of them are the blind spots, and all
three were **verified to convict** by reverting the hardening (3 of 17 go red). The hardening it
holds:

1. the seats block must carry `seatElements`/`renderingNothing`, i.e. must be the pod union;
2. seats on the ring with **no** measurable pod is a failure, not an empty loop;
3. the board's `if (m.board.rendered)` skip is cross-checked against painted hole cards, so
   "no board" is a **checked** skip on `table-empty` and the deposit dialog and a **failure**
   on a dealt hand. The module's own header claimed the latter in two places while doing the
   former on 4 of 24 shots.

---

<a id="h-52"></a>
### H-52 — high — nothing compared the guardian's committed `.did` to the guardian wasm — STATUS: FIXED (wave 12)

The guardian's entire safety claim is a statement about its **interface**: no `install_code`, no
`uninstall_code`, no `update_settings`, no `delete_canister`, no snapshot verb, anywhere on its
wire. Three gates assert it — the census, the name list and the Candid-driven sweep in
`controller_custody.rs` — and **all three parse the COMMITTED
`src/guardian_canister/guardian_canister.did`**. `scripts/check-candid.sh` covered
`lobby_canister`, `table_canister` and `history_canister` only, and `cmd_custody` ran
`build-guardian.sh` **without** `--did`.

A wave-12 critic compiled a real `install_chunked_code(mode = Reinstall)` into the guardian and
left the `.did` as committed: **all nine custody tests passed GREEN over a canister that can
wipe a funded table.** The same mutation *with* a regenerated `.did` goes red, which is why the
builder's own mutation test — which regenerated it — never saw the divergent case.

`check-candid.sh` now carries a guardian block (and a `--guardian-only` mode so `cmd_custody`
can run it without building three canisters it does not use), and `cmd_custody` runs it
**before** the tests, so every assertion below it is a statement about the module. Verified to
go red by removing one method from the committed `.did`.

---

<a id="h-53"></a>
### H-53 — high — the sealed-dealer spike's 36 tests were named by no target — STATUS: FIXED (wave 12)

[H-45](#h-45) a third time, and this one names itself. `tests/no_peeking/Cargo.toml` declares all
five `[[test]]` targets explicitly, under the comment:

> Named explicitly for the reason every target in `tests/money_safety/Cargo.toml` is named
> explicitly: an auto-discovered target is a target no human ever types.

And then no `dev.sh` target, no make rule and no CI job typed any of them. 36 tests — every
exported method of both spike canisters called as the controller, as an opponent, as a stranger
and anonymously, plus the canister-snapshot read that defeats the obvious fix. All green, ~30 s
once built.

Several are the [FINDING 23](SECURITY-FINDINGS.md#finding-23) pattern and **pass while a defect
in the spike is live**: `two_concurrent_try_advance_calls_pay_the_pot_out_twice`,
`a_zero_in_the_install_argument_reopens_the_whole_hole`,
`phantom_seats_give_the_operator_private_cards_from_the_live_deck`. Read the names before
reading `docs/NO-PEEKING-FEASIBILITY.md` §10 as a design that is ready.

`./scripts/dev.sh no-peeking` (and `make no-peeking`) runs it; `dev.sh test` runs it as step 8
of 8. Nothing in it is deployed, nothing is in `icp.yaml`, and both crates are detached from the
root workspace, so it cannot move a deployed module hash.

---

<a id="t-43"></a>
### T-43 — high — the frontend turned OFF query-signature verification, so any box on the path could rewrite any query reply — STATUS: FIXED (wave 14)

`createAgent()` in `src/cleardeck_frontend/src/lib/canisters.js` built every `HttpAgent` with

```js
verifyQuerySignatures: false,   // "Disable query verification for now - there may be subnet key issues"
```

`false` is not `@dfinity/agent`'s default. Somebody wrote it, speculatively (*"may be"*), and it
stayed. What it disables is the check that a query reply carries a signature from a node of the
subnet that hosts the canister. Without it there is nothing tying a reply to the IC at all: a
boundary node, an HTTP gateway, a corporate proxy or a compromised CDN edge can rewrite any
query reply this app makes and the client cannot tell.

Every uncertified-reply finding in this project is one class of attacker cheaper because of it.
[FINDING 42](SECURITY-FINDINGS.md#finding-42) needs a dishonest replica to substitute a table
canister id in `get_tables()`; with this line, it needed only a position on the wire.

**Fixed by restoring the library default, and MEASURED rather than assumed.** With
`verifyQuerySignatures: true` the whole app works against the local replica — the lobby list
(three `get_tables` + `get_player_count` + `get_max_players` queries), the table view and the
deposit modal all render, with **zero console errors**. Evidence:
`node tools/shots/repro-finding42.mjs`, `artifacts/finding42/evidence.json`
(`observed.consoleErrors: []`). The "subnet key issues" the comment speculated about did not
appear.

**What was NOT tested: mainnet.** This wave does not call mainnet (hard rule 1). Whoever deploys
next should open the lobby before announcing it. The failure mode is loud — "Failed to load
tables" — not silent.

**What this does not fix, stated because the opposite is the tempting conclusion.** A verified
query is still ONE replica's opinion, signed. It binds the reply to a node key; it does not put
the reply through consensus. A dishonest node can still answer `get_tables()` with any canister
id it likes and sign it. That is why the deposit address is rooted in `trustedTables.js` — the
ids the build was published with — and not in anything that arrives over the wire, verified or
not.

<a id="t-44"></a>
### T-44 — medium — the two warnings on the screen money leaves from are uncertified query replies, and their reassuring state renders nothing — STATUS: OPEN

`DepositModal.svelte` asks two questions before a player commits money, and both were added
because a real failure was invisible without them:

* **solvency** — [FINDING 35](SECURITY-FINDINGS.md#finding-35) / [E-70](#e-70): mainnet `table_1`
  is 2.00 ICP short of its own books and no surface of the canister said so.
* **cycle runway** — [E-55](#e-55): a canister below its freezing threshold rejects every update
  call at once, so every player loses access to their own money at the same instant.

Both are read with a `query` (`solvency.js` `readTableSolvency`, `cycleRunway.js`
`readCycleRunway`). A query is answered by one replica out of its own memory; no consensus is
involved and, since [T-43](#t-43), the reply is signed by that node and nothing more.

**The dangerous part is the rendering, not the read.** `SolvencyNotice` renders *nothing at all*
when the verdict is `covered`. So a replica that answers `covered` produces a deposit screen that
is pixel-identical to a healthy table — the warning does not go wrong, it goes absent, and
absence is the state a player reads as "fine". The runway block behaves the same way for a
comfortable answer.

This is the project's standing lesson one level up: **an instrument that measures nothing
passes.** Both instruments are also anchored INSIDE the canister they are judging, which is the
other standing lesson.

Two ways to close it, neither done here: read the pre-deposit verdict through an UPDATE call, so
it goes through consensus like the deposit itself; or say on the screen that the verdict is one
replica's answer and was not verified. The first is the real fix. The second is honest and costs
nothing.

<a id="t-42"></a>
### T-42 — high — the collusion detector accuses honest winning players and cannot tell them from cheats — STATUS: OPEN

Signal 2 of `tools/archive/lib/collusion.mjs` asks *"does A lose to B faster than A loses to
everybody else"*. **That is also the definition of "B is better than A's other opponents."** The
null is therefore false under ordinary skill variation, and under a false null the error rate
**rises toward 1 with sample size** instead of holding at alpha.

**Measured on real, uncoordinated canister hands.** 800 hands on the local `table_3` with three
bots that never coordinate and never see each other's cards — a calling station (VPIP 68.6%,
AF 0.00), the repo's own honest bot (VPIP 38.3%) and a tight-aggressive winner (VPIP 19.9%,
AF 9.85), realised at −101.3 / −39.1 / +140.4 bb/100. The shipped detector returned **two
`REVIEW` rows at q = 1.50e-4, both naming the winner as beneficiary**. The deliberately colluding
session returns **two `REVIEW` rows at q = 1.50e-4**. Same verdict, same count, same q, same
table shape.

**Why the shipped gate is green over it.** `selftest/synthetic.mjs` draws every player's
per-hand result from one and the same `drawResult()` — it has no per-player skill parameter at
all — so its 25 "honest" worlds contain **zero** of the variation this signal mistakes for a
leak. Add a skill ladder and a 200 bb/100 spread flags 5 of 25; hold an ordinary 60 bb/100
spread fixed and grow the sample and the flag rate goes 0/10 → 1/10 → 0/10 → **4/10** at 700 /
2,252 / 6,592 / 19,629 shared hands. The assertion behind *"at or below the stated alpha"* is
`assert(flagged / trials <= 0.12)`, which passes at 2.4× the alpha its own name and message
claim.

**Separately, the p-value is anti-conservative by construction:** it omits the sample's own
sampling variability, so on perfectly Gaussian data with no tail it fires at 14% where it claims
5% and at 2.4% where it claims 0.167%.

**Also corrected in the same pass, and not the same defect:** `docs/ARCHIVE.md`'s power table
quoted `sd = 3.70 bb/hand` and *"about two million hands"* for a 1 bb/100 leak. Running the
shipped `power.mjs` on the bundle the documented fetch command actually produces gives
`sd = 12.25` and **21,422,570** hands. 3.70 comes from a ~351-hand single-table slice that no
documented command emits.

**Disclosure, not fix.** The limitation is now stated at the top of
[ARCHIVE.md §6](ARCHIVE.md), in §9, in the gates table, and **in the tool's own output above
every Signal 2 table**, because a reader who never opens the document is the one who needs it.
Signals 1 and 3 are unaffected and are the ones with a mechanism no other room has. **The fix is
a null conditioned on opponent strength, not a tighter alpha**, and no Signal 2 row should be
shown to a player or used to restrict an account until it exists.

---

<a id="h-54"></a>
### H-54 — high — the fast gate's own timeout watchdog outlives the run and holds its output pipe open — STATUS: FIXED (wave 12)

```bash
with_timeout() {
  local secs="$1"; shift
  ( "$@" ) & local pid=$!
  ( sleep "$secs"; kill -9 "$pid" 2>/dev/null ) & local wd=$!
  wait "$pid"; local rc=$?
  kill "$wd" 2>/dev/null      # <-- kills the SUBSHELL, not the `sleep`
  ...
```

`kill "$wd"` kills the subshell. The `sleep` it forked is a **separate process**: it is reparented
to init and keeps running, and it still holds the **stdout and stderr it inherited**. So the gate
exits normally and anything reading its output through a pipe — `| tail`, `| tee`, `$( )`, a CI
log collector — blocks until that `sleep` expires, with **zero CPU on every process involved**.

**This is [H-42](#h-42)'s mechanism.** H-42 records the symptom exactly (*"hung for 33 minutes with
zero CPU on both the test binary and its own PocketIC"*) and diagnoses it as the money-safety
targets having no time bound. It is the opposite: the two settlement steps that DO have a bound are
the ones that leak, `900 + 300` seconds is the twenty minutes, and **all of it is after the gate has
already finished** — every suite has passed or failed and the summary line has been printed.

Measured this wave: `./scripts/dev.sh test` completed, its own process was gone, and `sleep 900`
sat at PPID 1 holding the pipe for another fourteen minutes while `tail -80` waited for EOF.

### Reproduced both ways, in four lines

```text
OLD, `with_timeout 300 sh -c 'echo hello; sleep 1'` piped to `cat`:
    hello
    STILL BLOCKED after 4s          and `sleep 300` is alive at PPID 1
NEW, the same command:
    hello
    returns in 1.2s                 0 leaked sleeps
```

### The fix, two independent halves because either alone is sufficient

* the watchdog subshell's own output goes to `/dev/null`, so a leaked child cannot hold the pipe;
* `pkill -P "$wd"` before `kill "$wd"`, so there is nothing left to leak — **children first**,
  because killing the subshell is what orphans the `sleep`.

**Why it is `high` and not cosmetic.** The primary gate appearing to hang for twenty minutes after
it has finished is how a gate stops being run. H-42 is the entry recording that it already happened.

---

<a id="e-99"></a>
### E-99 — high — a currency flip was accepted with an irreversible 5 ICP withdrawal still open — STATUS: FIXED (wave 14)

Measured on the same tree, by reverting `total_liability()` alone to its five-term sum:

```text
PRE-FIX   WITH A PAYOUT OPEN: verdict=CanPayEveryone owed=0 guard_liability=0
                              held=Some(500000000) diff=Some(500000000)
                              payouts=500000000
          flip to BTC with a payout in flight -> ACCEPTED currency now BTC

POST-FIX  WITH A PAYOUT OPEN: verdict=CanPayEveryone owed=500000000
                              guard_liability=500000000 held=Some(500000000)
                              diff=Some(0) payouts=500000000
          flip to BTC with a payout in flight -> REFUSED Refusing to change this
          table's currency from ICP to BTC while it still owes players 5.0000 ICP…
```

Note the pre-fix line twice over: a canister with zero escrow and a five-ICP withdrawal in
flight reported `owed = 0` and a **+5 ICP surplus**. That is
[FINDING 43](SECURITY-FINDINGS.md#finding-43) from a third direction, and it is why the fix is
one definition rather than a fifth term.

Gate: `cd tests/money_safety && cargo test --test solvency_definition -- the_currency_guard_refuses_while_a_payout_is_still_in_flight`.
The destructive operation is attempted FIRST in that test, before any assertion about the
report, so the conviction that fires is the flip itself and not a bookkeeping check on the way
to it.

<a id="e-96"></a>
### E-96 — low — money held for somebody the table cannot name renders NOTHING on the deposit screen — STATUS: OPEN


> **CORRECTION, wave 14 coherence pass.** This id is also being cited in
> `artifacts/screens/acknowledged-reds.json` to excuse the OPPOSITE symptom — the
> deposit screen's token census failing on money figures that DO render. One id
> cannot carry both. The census red is now **[E-101](#e-101)**; this entry is only
> about the silence.


**Found in wave 14 by RENDERING the shipped component, not by reading it.**
`tools/shots/test-solvency.mjs` exercises the interpreter; nothing renders
`SolvencyNotice.svelte`. Compiled for SSR and rendered against each reachable state, on the
numbers the wave-14 money-safety run actually produced:

```text
THE FIXED CANISTER, SHORT (context=deposit)
  ⛔ This table does not hold all the money it owes
     Owed to players      10.00 ICP (1000000000 e8s)
     Held on the ledger    9.00 ICP (899980000 e8s)
     Short by              1.00 ICP (100020000 e8s)
     Ledger reading 0s ago.
     THIS CANISTER CANNOT PAY EVERYONE IT OWES … 2.0000 ICP (200000000 e8s) of that is
     money named by unfinished incoming transfers …
     Do not deposit. …

THE FINDING 43 STATE — 1 ICP at the shared main account, credited to nobody
  (NOTHING RENDERS)
```

The second one is the gap. The canister is right and says so in words: it holds exactly what
it owes (`difference_e8s = 0`, verdict `CanPayEveryone`) **and** it names
`unattributed_at_main = 100000000` in `summary`. The component's rule 2 is *"silence is never
the answer"*, and its `visible` predicate is `state !== covered`, so the whole block —
summary included — is dropped. `interpretSolvency` does not extract `unattributed_at_main`
at all, so even an unconditional render would have nothing to show.

**Why it was not fixed in the same wave.** `SolvencyNotice.svelte` carries a note from its
author: these are MONEY FIGURES, and the moment they render they need a site in
`tools/shots/lib/chain-agreement.mjs` asserting them against the canister — a
`token-allowlist.mjs` rule is forbidden for money-shaped tokens by that file's own rule 5.
Adding an unasserted money figure to the deposit screen to fix an information gap would be
the trade this project keeps refusing. The fix is: extract `unattributed_at_main`, render an
informational (not critical) row when it is non-zero, and add the chain-agreement site in the
same change, with `./scripts/dev.sh shots` green.

<a id="e-98"></a>
### E-98 — high — wave 12 moved the sweep threshold and left every sentence about it behind — STATUS: FIXED (wave 14)

Wave 12 added the Rule-3 floor: `claim_external_deposit` used to sweep anything the ledger could
move (`balance > transfer_fee`) and now sweeps only at or above `min_external_deposit`
(`min_withdrawal + transfer_fee`). The floor is right. **Three player-facing statements were built
on the old threshold and none of them moved**, so for two waves this canister gave instructions it
would then refuse to carry out.

| | said | on a build where |
|---|---|---|
| the dust refusal | *"sending **2** e8s or more to the SAME address makes the whole balance claimable with claim_external_deposit()"* | 9,999 + 2 = 10,001, and the claim refuses at 10,001 |
| `deposit_custody_sentence` | *"call claim_external_deposit(), which moves it and credits you 0.0001 ICP"* | the claim refuses the entire band it was being said about |
| `DepositAddressCustody::sweepable` | `observed_amount > transfer_fee`, under *"true when `claim_external_deposit()` would sweep this amount right now"* | the sweep refuses everything from `fee + 1` to `min_external - 1` |

The third is [E-89](#e-89)'s second half and is written up there. The first two are why this entry
exists separately: they are not flags a machine reads, they are **the only instructions a stuck
player has**, and following them costs another ledger fee and ends in another refusal.

### Why nothing caught it, and what the gates do differently

`deposit_custody_sentence` exists because of
[FINDING 28](SECURITY-FINDINGS.md#finding-28) — it is the ONE place this prose is written, so the
four surfaces that quote it cannot diverge from each other. They did not diverge. **They agreed
with each other and disagreed with the code**, which a same-source check cannot see, and the two
tests that touch the dust path both top up by `2 * ICP` — comfortably over any floor — so the
figure the canister actually prints was never sent.

Both new gates DRIVE the sentence instead of reading it:

* `the_top_up_the_dust_refusal_names_actually_makes_the_balance_claimable` parses the number out of
  the refusal, transfers exactly that, and requires the claim to return `Ok`. It cannot agree with
  a wrong figure, because it uses the figure.
* `sweepable_and_refundable_predict_what_the_two_doors_actually_do` reads the flag, then calls the
  method, at `fee`, `fee + 1`, `min_external - 1` and `min_external`, and requires the prediction
  and the outcome to match.

Verified red against `git show HEAD:src/table_canister/src/lib.rs` in a `cp -Rc` tree: *"THE
CANISTER'S OWN INSTRUCTION DID NOT WORK. It said to send 2 e8s more to the same address; that was
done, the address now holds 10001, and the claim still refused."*

---

<a id="e-97"></a>
### E-97 — medium — the suite that produces the runway table prices an open browser tab as a heartbeat stream — STATUS: OPEN

`tests/money_safety/tests/cycles_runway.rs` is the only thing in this tree that MEASURES a table's
burn under load, and it is where every published runway figure originally came from. Its per-tab
rows are built like this:

```rust
for seats in [2u128, 6] {
    let burn = idle_per_day
        + net_per_hand.saturating_mul(500)
        + hb_per_player_day.saturating_mul(seats);
    print_runway_table(
        &format!("500 hands/day with {seats} tabs open (heartbeats at 10 s)"),
        burn,
    );
}
```

`hb_per_player_day` is the 10-second heartbeat and nothing else. That is
[E-92](#e-92)'s arithmetic, expressed in code rather than in a comment, and it is the reason the
same wrong number reached six other files: they were all copying this.

Measured in wave 14 on the local replica (`tools/cycles/tab-burn.mjs`, control-differenced):

| what a tab actually costs | per day |
|---|---|
| the heartbeat term this suite uses | 0.0618 T |
| the FIXED client, measured on a table nobody is playing at | **0.0572 T** |
| the FIXED client at its policy ceiling | **0.1929 T** |
| the client this suite was written beside (500 ms poll) | **1.1380 T** |

So even after [E-92](#e-92) the ceiling row reads about **3x low**, and against the client that
existed when the suite was written it read **18x low**.

**What is NOT wrong here.** The rows that involve no tabs — idle, and N hands/day — are sound. This
suite's idle figure (0.0442 T/day, PocketIC) and wave 14's independent local-replica control
(0.0411 T/day, taken twice thirty minutes apart and agreeing to 0.011%) are within 7% of each
other, which is about what two different execution environments should differ by. The per-unit
costs it publishes for a hand, a deposit and a withdrawal are carried forward unchanged into
`tools/cycles/burn-table.json` and labelled as inherited from this suite.

**Why it is not fixed here.** It is another owner's suite, and the honest correction is to add a
measured per-tab term to the derivation and re-run it under PocketIC — not to edit the format
strings so the labels stop being false. Replacing one unmeasured constant with another is how the
226-day figure got written in the first place.

Everything OUTSIDE that suite now reads `tools/cycles/burn-table.json`, which carries the measured
per-tab prices, so nothing a player or an operator sees depends on this any more.

---

<a id="e-100"></a>
### E-100 — high — the cycles monitor never read the recent burn rate, and its own selftest could not tell — STATUS: FIXED (wave 14)


> **CORRECTION, wave 14 coherence pass.** The headline *"225 days reported for a
> table burning 2.05 T/day: a 56x error"* is a FIXTURE result, not an observed
> fleet result. Run against the live local canisters with only the parser line
> reverted, the **days column is byte-identical** both ways (table_3 = 23 days),
> because `get_cycle_status` already reports `runway_days` derived from the recent
> rate and `parse_status` takes `min(stated, derived)`. What the fix actually
> restored is the script's INDEPENDENT cross-check, dead since it was written, and
> the printed burn rate — which only diverges while the recent rate differs from the
> lifetime average. Re-measured by the coherence pass on an idle local fleet, the
> two parsers print **identical** output on all four tables, days and burn alike;
> the divergence the wave recorded (table_3 0.131 → 2.210 T/day) was visible only
> while that canister was hot from the burn matrix. The defect and the gate are
> real; the severity narrative overstated what was observed on a real canister.


`scripts/cycles-runway.sh` exists because a **lifetime burn average reads HIGH on a table that has
just got busy** — that is [E-55](#e-55)'s reopened half, and the script's own comment says so:

```python
# THE PESSIMISTIC RATE. A lifetime average alone reads high on a table that has
# just got busy, which is precisely when the number is needed.
burn = max(lifetime or 0, recent)
```

`recent` was always `0`.

```python
recent   = num('recent_burn_per_day') or 0     # num() matches `field = <digits>`
```

`recent_burn_per_day` is `opt nat` on the interface, so what the canister actually puts on the wire
is

```
recent_burn_per_day = opt (2_604_344_185_140 : nat)
```

which `num()` cannot match. It returned `None`, `or 0` turned that into zero, and `max()` therefore
chose the lifetime average on **every real canister, every run, since the script was written**.

### How it was found, and why nothing found it earlier

Found in wave 14 by running the monitor against a live local canister that had just been driven hard
by the [E-92](#e-92) burn matrix. The canister's own gauge said `recent_burn_per_day = opt
(2_604_344_185_140 : nat)` — 2.60 T/day — and the script printed `burning 0.133 T/day`, its lifetime
figure. Reproduced directly:

```
num('recent_burn_per_day')   = None
num('observed_burn_per_day') = 132777379425
```

Nothing found it earlier because **the selftest's fixtures were the wrong shape**. Every one of them
wrote the field as a bare `nat`:

```
recent_burn_per_day = 499_412_781_032 : nat
```

No module has ever emitted that. So the check named *"a busy table whose lifetime average still says
225"* — the one check in the file that exists specifically to prove the recent rate wins — passed
against a canister that does not exist, and five green lines proved the parser worked on fiction.
**An instrument that measures nothing passes.**

### What it cost, measured on the pre-fix parser

The fixture is now written in the wire shape. Replaying it against the parser as it was:

```
   ✗  a busy table, recent burn as `opt nat` -- THE SHAPE ON THE WIRE -> parsed 225 days, expected 4
```

**225 days for a table burning 2.05 T/day.** The error is 56x and it is in the direction that makes a
canister look safe.

It did not produce a wrong ANSWER on the local fleet only because of a second mechanism: the script
also takes `min(stated runway_days, derived)`, and the canister's own `runway_days` is computed from
the recent rate, so the canister rescued the monitor. That is luck, not design — the whole point of
recomputing locally is that a canister reporting a stale or optimistic `runway_days` must not be able
to talk the monitor round, and against such a canister this parser would have believed it.

### The fix

`opt_num` first, `num` second, absent last — the optional shape the module emits, the bare shape an
older module would emit, and absent still meaning "fall back to the lifetime figure", which was
always the intent. The selftest now carries **both** shapes as separate cases, so neither can rot
unnoticed.

**The JS reader was never affected.** Candid decodes `opt t` to `[]`/`[value]` and
`src/cleardeck_frontend/src/lib/cycleRunway.js` reads it through `optBig`, which handles the array
and tolerates a bare value. `tools/shots/test-cycle-runway.mjs` fixtures it as `[44_247_843_312n]`.
The defect was in the text parser only — which is exactly why a shell script re-implementing a
decoder deserves fixtures taken from a real reply.

**Gate:** `./scripts/cycles-runway.sh --selftest`, which `./scripts/dev.sh cycles` runs before it
touches a network. Red on the pre-fix parser with the wire-shaped fixture, exit 1.

---

<a id="e-89"></a>
### E-89 — high — the fuzzer at its own defaults is RED, and the red is FINDING 31 — STATUS: FIXED (wave 14)

> ## WAVE 14 — FIXED. A REFUSAL IS NOT A REMEDY, AND FOR ONE WAVE THAT IS ALL THIS WAS.
>
> `./scripts/dev.sh fuzz-default` is GREEN, seed `0xc1ea2dec0002` included, in **49.2 s** against
> the 303.0 s the red run took (the difference is one shrink pass that no longer has anything to
> shrink). The drain transcript now contains the line that was missing:
>
> ```text
> ledger intent 17 OPENED:  refund 10002 e8s for lpoz5-…-oae
> ledger intent 17 SETTLED: refund of 10002 e8s from the deposit subaccount of lpoz5-…-oae
>                           to their own wallet at block 40
> ```
>
> **Why the wave-12 Rule-3 floor did not close it, which was the question.** The floor was right
> and it was only half the job. `min_external_deposit()` stops the canister CREDITING an escrow
> balance it could never pay out — but the money it refuses to sweep is sitting in an account this
> canister owns, at an address this canister published, and the only instruction the refusal could
> give was *"send at least X more to the SAME address"*. That asks a player to spend a second
> ledger fee to rescue the first, and if they decline, the canister holds their money for ever
> while every arithmetic invariant stays silent. **Refusing to take money is not the same act as
> giving it back**, and only the second one empties the account.
>
> The arithmetic says the two doors are not equivalent, which is why this needed a new one:
>
> | route | the player receives | needs a balance of |
> |---|---|---|
> | sweep, then `withdraw` | `balance - 2*fee` | `> 2*fee` |
> | `refund_external_deposit()` | `balance - fee` | `> fee` |
>
> So the refund reaches **every amount the ledger can move**, which is the most any canister can
> promise, and it costs one fee rather than two. Neither its source nor its destination is a
> parameter: both are derived from `msg_caller()`, so it can drain no account but the caller's own
> and pay no account but the caller's own, and it never touches `BALANCES`.
>
> ### The second defect in this entry — the tolerance — is fixed as a tolerance, not as a number
>
> `UNMOVABLE_DUST_E8S` is gone. [`DrainReport::stranded_e8s`] asks the question **per account**:
> after every legal exit is driven to exhaustion, the only money that may still be owed is money
> the LEDGER itself cannot move, and that is a fact about one account at a time. An escrow row or
> a deposit subaccount above one transfer fee is stranded; chips and the pot count in full at any
> size, because `cash_out` needs no transfer at all. Measured, on the four cases that separate the
> two rules:
>
> | left after the drain | old rule | new rule |
> |---|---|---|
> | one address holding 19,999 | **silent** | 19,999 |
> | two addresses holding 10,001 | 20,002 | 20,002 |
> | two at 10,000 (dust) + one at 15,000 | 35,000 | **15,000** |
> | one chip in a seat | **silent** | 1 |
>
> ### A third defect, found by driving the instruction instead of reading it
>
> The dust refusal named a top-up of `fee + 1 - amount`. That figure was correct until the wave-12
> floor moved the sweep threshold and was never updated, so a player holding 9,999 e8s was told to
> send **2 more** — and at 10,001 the claim refuses again, with a different message, for a
> different reason. Driven on the pre-fix build: *"It said to send 2 e8s more to the same address;
> that was done, the address now holds 10001, and the claim still refused."* It now names
> `min_external_deposit - amount`, and the gate sends exactly the number the canister prints.
>
> ### Every gate verified RED first, against `git show HEAD:src/table_canister/src/lib.rs`
>
> A `cp -Rc` tree with the new tests and the committed canister: **6 passed, 6 failed** — the six
> pre-existing `deposit_floor` tests still green, all six new ones red, on
> `CanisterMethodNotFound 'refund_external_deposit'`, on the missing `minimum_deposit` field, on
> `20000 e8s went to the address this canister published and 0 came back to their wallet`, and on
> the top-up instruction above. The four tolerance tests were verified red separately, with the
> old aggregate rule restored in the same tree. And with the new instrument against the OLD
> canister the fuzzer still convicts, naming the account:
> *"20002 of that is in accounts the LEDGER could still move: 20002 e8s in deposit address of
> lpoz5-…-oae"* — so the drain did not go green by going blind.
>
> **What is NOT closed by this.** Money at or below one transfer fee is still unrecoverable, by
> this canister or by anybody, and that is arithmetic rather than a defect
> ([FINDING 11](SECURITY-FINDINGS.md#finding-11) / [E-12](#e-12)). What changed is that the
> canister now says so on the address surface **before anything is sent**, and publishes
> `minimum_deposit` — a number that cannot strand the player who follows it exactly.


`./scripts/dev.sh fuzz-default` — the invocation [H-28](#h-28) exists to make sure somebody runs —
fails, and `./scripts/dev.sh test` fails with it. **Reproduced twice, identically**, 2026-08-07:

```text
money-fuzz: seed 0xc1ea2dec0001 finished: 3 hands, 5 upgrades, 0 blocking finding(s), worst stranded 0 e8s
money-fuzz: seed 0xc1ea2dec0002 finished: 7 hands, 3 upgrades, 1 blocking finding(s), worst stranded 20002 e8s
money-fuzz: seed 0xc1ea2dec0003 finished: 4 hands, 3 upgrades, 0 blocking finding(s), worst stranded 0 e8s
test result: FAILED. 0 passed; 1 failed   (389.59s, and 392.01s on the first run)
```

### The minimal reproducer is five operations

```json
[{"ExternalDepositThenClaim": {"actor": 0, "amount": 10001}},
 {"FundAndSeat": {"actor": 3}},
 {"FundAndSeat": {"actor": 0}},
 {"ExternalDepositThenClaim": {"actor": 3, "amount": 10001}},
 {"StartNewHand": {"actor": 3}}]
```

Two players each send **10,001 e8s** to the deposit address the canister publishes for them, and
claim it. Then every player-side exit is driven to exhaustion — `cash_out`, `withdraw` down to the
last e8, ten rounds of it — and the transcript ends:

```text
pre:     deposit address of 74yuz-…-dae holds 10001 (sweepable=true)
pre:     deposit address of toldy-…-aae holds 10001 (sweepable=true)
round 0: cash_out 200000000
round 0: withdraw 1005020001 -> balance 38
…
round 9: deposit address of 74yuz-…-dae holds 10001 (sweepable=true)
round 9: deposit address of toldy-…-aae holds 10001 (sweepable=true)
after every legal player-side exit was driven to exhaustion, the canister still owes 20002
of the 4490040003 e8s it started with, and no player call can move it
```

### It is FINDING 31, and this is the gate FINDING 31 never had

10,001 e8s is inside the dead band
[FINDING 31](SECURITY-FINDINGS.md#finding-31) names, `(10_000, 20_000]`: above the ledger fee, so
the canister's own probe says **`sweepable=true`**, and below the withdrawal floor once the sweep
has taken the fee out of it, so `withdraw` refuses what is left. FINDING 31's register row says
*"Not one of `deposit_floor.rs`'s six tests sends anything to a deposit subaccount"* and its gate
column is `—`. **The fuzzer sends one, at its own default seeds, and convicts.**

### Why it fires at 20,002 and not at 10,001, which is a second defect in the instrument

`check_drain` tolerates `owed_after <= UNMOVABLE_DUST_E8S`, and that constant is documented as
*"`Currency::ICP.min_withdrawal()` in the canister, plus one fee"* — **a per-account floor**. It is
compared against the **aggregate** owed across every account. So one stranded player is silently
tolerated and two are not: the verdict depends on how many seats happen to be holding dead-band
dust rather than on whether any of it is recoverable. At 9-max the same defect can strand nine
times as much and the check still uses one account's floor.

Both readings are true at once, which is why this entry is separate from FINDING 31: **the money is
genuinely unreachable** (FINDING 31, per account), **and the tolerance is dimensionally wrong**
(per account, applied to a total).

### Pre-existing at `134550e`, and stated as such rather than assumed

Wave 12 changed **no canister source** — `git status` over `src/table_canister`,
`src/lobby_canister`, `src/history_canister`, `src/poker_core`, `Cargo.toml`, `Cargo.lock`,
`rust-toolchain.toml`, `recipes/` and `Dockerfile` is empty — and **no fuzzer source**:
`tests/money_safety/src/fuzz.rs`, `invariants/` and `documented.rs` are untouched, and the only
change under `tests/money_safety/src/` is additive new functions in `wasms.rs` that the fuzz binary
never calls. The report records the same `table_wasm_sha256` the gate builds. **This red is what
`main` does today.**

### What it would take to make the gate green, and why neither was done here

1. **Fix FINDING 31.** It is a `src/table_canister` change, it moves all six deployed module
   hashes, and it therefore requires a mainnet redeploy. That is the lead's decision, not a
   reconciliation pass's.
2. **Add a bounded entry to `tests/money_safety/src/documented.rs`.** That file's own header says
   *"Adding an entry is admitting a defect is shipping"*, and the register is deliberately EMPTY.
   Tolerating a live fund-loss defect to make a gate green is the same mistake as
   [D-14](#d-14) in a different costume, and the bound would have to grow with seat count to be
   correct at all.

**Left RED on purpose, with an id.** A gate that is red about the top open `critical` is doing its
job; the thing that must not happen is for it to be red about it silently.

---

<a id="e-91"></a>
### E-91 — high — the archive's writer picks half of the hand's name, and after E-71 that silently DELETES another table's hand — STATUS: FIXED (wave 13)

**THE NINTH CROSS-AGENT DEFECT. It is a regression this wave introduced, in the wave whose own
source comments cite [E-49](#e-49) as the failure being avoided.**

`record_hand` authorised the CALLER and never bound `record.table_id` to it:

```rust
let is_authorized = state.authorized_tables.contains(&caller) || state.admin == Some(caller);
if !is_authorized { return Err(...) }
insert_hand(&mut state, record)      // record.table_id is whatever the writer said
```

[E-71](#e-71) then narrowed the de-duplication key from `(table_id, hand_number, seed_hash)` to the
hand's NAME, `hand_uid = "<table_id>:<seed_hash>"`. Those two facts together are the defect: the
writer chooses `table_id`, and the other half of the name — the shuffle commitment — **is public**.
It is published before any card is dealt and it is on the player's screen while the hand runs.

### What changed, and it changed in the silent direction

| | HEAD (`a65868e`) | after E-71 |
|---|---|---|
| key | `(table_id, hand_number, seed_hash)` | `(table_id, seed_hash)` |
| forged record naming another table | stored as its OWN record under its own `hand_number` — **visible** | shares the victim's whole name |
| the genuine hand that arrives after it | stored | **de-duplicated away, `Ok(forgery_id)` returned** |
| records surviving | 2 | **1, and it is the forgery** |

So filing a forgery first deletes a genuine hand from an archive whose entire promise is that it is
permanent and append-only, with an `Ok` on the way out and no counter moving. Reachable by any of
the four authorised table canisters.

**Every instrument built this wave reads green over it**, and that is the eight-times-repeated
signature: `get_archive_integrity` recounts NAME COLLISIONS and there is no collision — there is one
record where there should be two. `index_disagrees_with_records` compares the index against the
records and they agree. `check_recorded_hand` is asked about a record that is present. The failure
is an ABSENT record, and nothing in the tree counts absences.

### The fix

`record_hand`'s authorisation moved into `may_record`, a pure function, and it now ends:

```rust
if state.admin != Some(caller) && record.table_id != caller { return Err(...) }
```

`record_hand_to_history` sets `table_id = self_principal()`, so no honest table can fail it. **The
admin stays exempt and that is stated rather than assumed**: the archive's admin is its controller
and can rewrite the whole canister by reinstalling it
([FINDING 23](SECURITY-FINDINGS.md#finding-23)), so refusing here would be a claim rather than a
control. `the_admin_is_still_allowed_to_record_for_a_table` pins the exemption so that removing the
binding can never be mistaken for "the admin path was already closed".

### The gate, and that it goes red

`cargo test --workspace` (step 1 of 8 in `./scripts/dev.sh test`), five host tests in
`history_canister::retention_tests`. Neutering the binding to `if false` and re-running:

```text
test retention_tests::a_table_may_not_record_a_hand_naming_another_table ... FAILED
test result: FAILED. 22 passed; 1 failed
```

`two_records_sharing_one_name_collapse_to_one_which_is_why_the_binding_exists` measures the
mechanism rather than arguing it: two records with one name, one survives, and what survives is the
one with `total_pot = 1` rather than the hand that was played.

### Why the split is worth keeping

`may_record` and `insert_hand` are both split out of the canister entry point for the same reason,
now written down twice: **a rule that decides whether one table can delete another's evidence must
not be reachable only through a deployed replica.**

---

<a id="e-92"></a>
### E-92 — high — the render rate was driving an UPDATE loop, and it was the largest single cost of running this game — STATUS: FIXED (wave 14)


> **CORRECTION, wave 14 coherence pass.** The per-tab prices labelled *typical* were
> measured on an EMPTY table: `artifacts/cycles/fixed-{1,3,10}tab.json` contain no
> `check_timeouts` key at all, so the clock nudger fired zero times in every cell
> `0.0572 T/day` derives from. Those cells are then multiplied by 6 and 10 in the
> *"500 hands/day, N tabs open (typical)"* rows — rows that by definition describe a
> table where the nudger fires between every hand. Filed as
> **[H-62](#h-62)**; measured correction ≈0.832 T/day for ten tabs (61 days on
> 51.4 T) rather than 0.6974 (73 days). The LEGACY-vs-FIXED ratio, which is what
> this entry is about, is unaffected. Separately, nothing gates the link between
> `+page.svelte` and the policy at all: **[H-58](#h-58)**.


Wave 13 measured the cost of a hand, a deposit, a withdrawal and the frontend's 10-second
heartbeat, and concluded that *"six open browser tabs cost more than eight times what the on-chain
clock costs"*. There was a second per-tab loop, it was not measured, and it was the larger of the
two by about a factor of twenty: **1.0841 T/day per tab against the heartbeat's 0.0539**, both
measured against the same control below.

```js
// src/cleardeck_frontend/src/routes/+page.svelte   (before this wave)
const POLL_INTERVAL = 500;
pollInterval = setInterval(loadTableState, POLL_INTERVAL);

async function loadTableState() {
  ...
  const timeoutResult = await tableActor.check_timeouts();   // FIRST statement
```

```rust
// src/table_canister/src/lib.rs
#[ic_cdk::update]
fn check_timeouts() -> TimeoutCheckResult {
```

`table_canister.did` declares it `check_timeouts : () -> (TimeoutCheckResult);` — no `query`. So
every open tab drove an **update** call in a 500 ms loop, forever, whether or not anything was due.

### MEASURED, not derived

The reason this survived is that every cycles figure in this repository was **derived**: somebody
added up the price of the things they remembered the frontend does. A derived figure omits whatever
the deriver forgot. So this entry does not derive anything.

`tools/cycles/tab-burn.mjs` opens N simulated tabs against a real table canister on the local
replica, drives exactly the call pattern the page drives — the same 500 ms poll with the same
re-entrancy guard, the same 10 s heartbeat, the same 5 s balance refresh, one Ed25519 identity per
tab — and reads the canister's own cycle balance before and after. The full method, and its two
stated confounds, are in that file's header. `./tools/cycles/run-matrix.sh` re-runs the whole
matrix.

**The control was taken twice, at the start and at the end of a 30-minute matrix, and came out at
41,141,210,251 and 41,145,603,927 cycles/day — 0.011% apart.** Every marginal figure below is a
difference against it.

| tabs | client | measured burn | marginal per tab | update calls in 132 s |
|---|---|---|---|---|
| 0 | — (control) | **0.0411 T/day** | — | 0 |
| 1 | 500 ms poll | 1.1791 T/day | **1.1380 T/day** | 251 |
| 3 | 500 ms poll | 3.4308 T/day | 1.1299 T/day | 754 |
| 10 | 500 ms poll | **9.7618 T/day** | 0.9721 T/day | 2,161 |
| 1 | fixed | 0.0951 T/day | 0.0539 T/day | 12 |
| 3 | fixed | 0.2128 T/day | 0.0572 T/day | 36 |
| 10 | fixed | **0.5799 T/day** | 0.0539 T/day | 120 |
| 10 | fixed, jammed table | 0.8947 T/day | 0.0854 T/day | 190 |
| 1 | fixed, policy ceiling | 0.2340 T/day | 0.1929 T/day | 42 |
| 10 | fixed, policy ceiling | 1.9333 T/day | 0.1892 T/day | 420 |

One `check_timeouts` costs **6,889,049 cycles**, averaged across every cell, which agrees with
wave 13's independently-measured 6,573,911 to within 5%.

**Ten open browser tabs on one table burned 9.76 T/day.** Add 500 hands a day and it is
**11.5 T/day: zero days on the 10 T [E-55](#e-55) headlines, four days on the 51 T the fixtures
hold.** The freezing reserve — 28,954,245,000 cycles, nominally thirty days — was worth
**four minutes**.

### The direction was the dangerous one

[E-55](#e-55) was reopened in wave 13 precisely because `runway_days` divided by a lifetime average
and therefore READ HIGH. This was the same failure one level up, in the numbers a human reads, and
it was baked into `FALLBACK_BURN_PER_DAY` — the floor the CI monitor alarms on when a canister
cannot yet measure its own burn. That constant was **four times too low**.

**What was never affected:** the canister's own `get_cycle_status`. Its sliding window measures real
consumption, so it always included these calls. Mid-matrix, `table_3` reported
`recent_burn_per_day = 605,243,690,623` and dropped its own `runway_days` from 1,035 to 84 while
every document in the tree still said 0.0442 T/day. **The gauge was right and the documents were
wrong**, which is the good half of this: the only instrument that was not a transcription saw it.

### Why it survived every instrument this project had

It is not a bug — the code does what it says. It is not a test failure — the game works. It is not a
lint — the call is well-formed. The only thing that makes it wrong is a fact in one file (whether
the Candid declares that method `query`) joined to a fact in another (how often the method is
called). **Nothing joined them.** `tools/shots/test-poll-updates.mjs` now does.

### The fix: two loops, because they answer to two different rates

Reading the table is a QUERY and its rate is the render rate. Advancing the clock is an UPDATE and
its rate is the rate at which DEADLINES ARRIVE, which has nothing to do with how often a browser
repaints. The canister already advances its own clock on an on-chain timer (`schedule_next_wake`
arms a precise wake at the next deadline, with a 30 s watchdog behind it), so a tab is a backstop,
not the engine. It has to fire for exactly three things, and `$lib/clockNudge.js` names them:

1. **An action clock at zero on a hand still in progress** — the on-chain timer is late, and a
   client is the only thing that can rescue the table.
2. **Between hands with two or more dealt-in seats.** This one is load-bearing rather than a
   backstop: `advance_table_clock` deliberately does not arm a wake for `auto_deal_at` when the
   table could actually deal, so without a client asking, `AutoDealReady` is returned to nobody and
   no hand ever starts.
3. **A hand nothing can move** — each call is one counted stall opportunity, and three across the
   grace period is what keeps `abandon_stuck_hand` reachable when a timer is dead
   ([FINDING 15](SECURITY-FINDINGS.md#finding-15)).

Conditionality alone is still an unbounded loop — a table jammed between hands is "due" forever — so
every decision passes a floor of 2 s that **widens to 30 s whenever two consecutive calls see the
table in the same state**, and snaps back the moment it moves. 30 s is `CLOCK_WATCHDOG_SECS`: a tab
backing up a lost timer has no reason to be faster than the timer's own watchdog.

The tempting alternative — reset the floor whenever the reply is *actionable* — is wrong and
measurably so. A jammed table answers `AutoDealReady` to every call while `start_new_hand` keeps
failing, so that rule holds the floor open for as long as the jam lasts: **7,200 calls over four
jammed hours against 480.** Both figures come from `tools/shots/test-clock-nudge.mjs`, which was
written first and caught the rule before it shipped.

### One source for the numbers

The table above used to exist in six places: this register, `scripts/cycles-runway.sh`,
`$lib/cycleRunway.js`, `DepositModal.svelte`, `WithdrawModal.svelte` and
`.github/workflows/cycles-monitor.yml`. Six transcriptions, no cross-check, all six wrong the same
way. There is now one generated file — **`tools/cycles/burn-table.json`** — written from the
measurements by `tools/cycles/build-burn-table.mjs`. `cycles-runway.sh` reads its fallback rate from
it and **prints the whole table at the top of every run**, so the figure a human sees is the figure
the tooling is using. The other five carry no numbers at all any more.

It publishes **three** per-tab prices rather than one, because any single number here is a lie:
the legacy poll (the comparison), the fixed poll measured on a table nobody is playing at (the
flattering figure), and the fixed poll pinned at its floor all day (the ceiling, unreachable by an
ordinary game). `fallback_burn_per_day` is built from the **ceiling**, because an alarm threshold is
the one place the pessimistic figure is the right one.

**A consequence, and it is not a comfortable one:** under the corrected floor, a canister that
cannot measure its own burn now needs **123.2 T** to clear the 60-day warning. 51.378 T — what the
local fixtures hold, and roughly what the mainnet tables hold — no longer clears it. That is not the
instrument being pessimistic. It is the instrument having stopped being wrong.

### The gates, and what each is red on

| gate | red on |
|---|---|
| `node tools/shots/test-poll-updates.mjs` | the pre-fix tree, both assertions: `setInterval(loadTableState, 500)` reaching `check_timeouts()`/`start_new_hand()`, and 181,440 worst-case updates/day/tab over a 60,000 budget (exit 1). Also red if `MIN_GAP_MS` or `TICK_MS` is lowered under 2 s, and red if `tab-burn.mjs` stops pricing the same three periods the page runs — verified by drifting its `POLL_INTERVAL` to 1000 ms, because a measurement of a client this repo does not ship is how a number stops being true without anybody editing it |
| `node tools/shots/test-clock-nudge.mjs` | the "actionable replies reset the floor" rule (0.0607 T/day/tab against a 0.02 T budget), and a policy that is always due. Carries its own control: a policy-free loop must fail the same budget, and if it ever passes, the budget is measuring nothing |
| `node tools/shots/test-burn-table.mjs` | a missing or self-inconsistent `burn-table.json`, a hardcoded fallback in `cycles-runway.sh`, and any reappearance of a retired burn figure in code |

### What is NOT closed here

`tests/money_safety/tests/cycles_runway.rs` still builds its "with N tabs open" rows as
`idle + hands + heartbeats x seats` — it prices a tab as a heartbeat stream, which is the arithmetic
this entry is about, in code rather than in a comment. Filed as [E-97](#e-97) rather than fixed
here: it is another owner's suite and correcting it means re-running its measurement, not editing
its labels.

---

<a id="e-93"></a>
### E-93 — low — `history_canister.wasm` changes with the `-p` set of the build that produced it — STATUS: OPEN

Identical source, identical directory, identical toolchain, `cargo clean -p history_canister` before
each:

```text
cargo build -p history_canister --target wasm32-unknown-unknown --release
  history_canister.wasm  ea701f773853c36f8c895639fd0bab2b5ded49071fcd1ddafb8ec7d60f862e15

cargo build -p table_canister -p history_canister --target wasm32-unknown-unknown --release
  history_canister.wasm  9f511048cdada0b9bbfe3548c5436ef67a89fb0d176a8c3f31db84285fc3db2d
  table_canister.wasm    3890a6d4a1861343df40c08772979171bc15fed8c51102c81f3c849346b29a0d
```

`table_canister` is `3890a6d4…b29a0d` under **both** invocations, so this is not nondeterminism in
the compiler: it is `cargo` unifying dependency FEATURES across every package named in one
invocation. `table_canister` already pulls the union, so it does not move; `history_canister` does.

### THE REPOSITORY HAS BOTH INVOCATIONS, so this is live and not theoretical

* `recipes/rust-reproducible.hbs` line 77 — the DEPLOY path —
  `cargo build --package {{ package }} --target wasm32-unknown-unknown --release --locked`.
  One package. Produces `ea701f77…`.
* `tests/money_safety/src/wasms.rs` — every PocketIC harness — `-p <one canister>`. Same shape.
* `scripts/check-candid.sh` line 142 — the gate that compares each committed `.did` against
  *"the interface exported by the built wasm"* — `cargo build --locked --release --target
  wasm32-unknown-unknown`, **the whole workspace at once**. Produces `9f511048…`.

So `target/wasm32-unknown-unknown/release/history_canister.wasm` **is whichever shape the last gate
you ran left there**. Measured on this tree at the close of wave 13: after `dev.sh test` +
`check-candid.sh` the file was `9f511048…`, and a `cargo build -p history_canister` immediately
afterwards rewrote it to `ea701f77…`.

No gate is currently wrong because of it — the exported Candid interface is the same either way, so
`check-candid.sh` is green, and every harness rebuilds before it reads — but "the artifact's identity
depends on which command ran last" is not a property this project is allowed to have.

The wave-13 two-path check passes when the invocation is held fixed: the working tree and a `cp -Rc`
clone agree to the byte on both modules (`table_canister 3890a6d4…`, `history_canister ea701f77…`).

### Why it is not `low`

The vendored recipe exists because *"an independent auditor could not match the deployed module hash
to ANY commit in this repository"*. A stranger reproducing a build is not reading the recipe's line
77; they type what looks obvious. Building two canisters in one command is obvious, and it produces
a module that matches nothing, which reads as **"this canister is not the code"** — the exact false
alarm the recipe was written to end.

**The fix, when somebody takes it:** make `scripts/check-candid.sh` build package by package like
everything else, say so where a verifier reads it (README's verification section), and have
`scripts/verify-build.sh` assert that it built one package per invocation.

---

<a id="e-94"></a>
### E-94 — medium — the read-only guard blocked the exact incident and nothing near it — STATUS: FIXED (wave 13)

`scripts/assert-read-only.sh` exists because the workflow it replaced base64-decoded
`secrets.IC_DEPLOY_IDENTITY` into `/tmp` every day to read one number. The guard it shipped with
matched that string, and generalised from it badly.

**What went straight through, measured on hostile files before the fix — each printed
`ok … read-only, anonymous` and exited 0:**

| planted in a workflow | why it passed |
|---|---|
| `${{ secrets.DEPLOY_KEY_PEM_BLOB }}` | `CREDENTIAL` was `secrets\.IC_` — any secret not named `IC_*` |
| `icp identity use monitor` | not `identity import` and not `--identity` |
| `icp canister create` | not in `MUTATING`'s verb list |
| `icp canister migrate-id` | same |

The shape is the one this project keeps finding: **a deny list built from the incident that
happened rather than from the property being enforced.** The property is "this job needs no
credential of any name and changes nothing", so the patterns are now the whole write half of the
CLI surface this repo touches and every `secrets.` reference.

`--selftest` carries all four as probe lines and requires 5 of 5 hits on each pattern, so the guard
proves it can see them before it judges anything. Re-run against the two real files it protects:
both still `ok`.

### What is still open, and is stated rather than fixed

**It is not transitive.** The guard reads the workflow; it does not open the scripts a workflow
invokes. `run: ./scripts/anything.sh` is unexamined, so read-only-ness stops at the YAML. Making it
transitive means resolving arbitrary shell, which is a different piece of work; until somebody does
it, the honest reading of a green result is *"this workflow file cannot deploy"*, not *"this job
cannot deploy"*.

---

<a id="e-95"></a>
### E-95 — high — the CI frontend job cannot succeed, so main's pipeline is permanently red — STATUS: FIXED (wave 13)

Two things landed in different waves and were never run against each other.

`.github/workflows/ci.yml`, before:

```yaml
      - name: Build frontend (declarations are committed under src/declarations)
        run: npm --workspace src/cleardeck_frontend run build
        env:
          DFX_NETWORK: ic
          # Placeholder IDs: the bundle resolves real IDs at deploy time from .env.
          VITE_CANISTER_ID_LOBBY: aaaaa-aa
          VITE_CANISTER_ID_HISTORY: aaaaa-aa
```

`src/cleardeck_frontend/vite.config.js`, after the mainnet-id cross-check landed — the mirror of
[T-01](#t-01), added so a bundle that says MAINNET cannot talk to a canister nobody named:

```text
Error: ABORT: building for MAINNET, but the canister ids do not match the roles in
.icp/data/mappings/ic.ids.json:
  LOBBY=aaaaa-aa but .icp/data/mappings/ic.ids.json says lobby=kpfcd-…-cai
  HISTORY=aaaaa-aa but .icp/data/mappings/ic.ids.json says history=kggj7-…-cai
```

Reproduced by running the job's exact command locally. **A placeholder id can never satisfy that
check**, so the job cannot pass, and it has not: main's last CI runs failed on `Build frontend`.

### Why it is `high` and not `medium`

Because of what was merged on top of it. [H-23](#h-23) put the fund-safety suites into this same
pipeline in this same wave — the suites whose whole purpose is that somebody looks when they go red.
**They arrived on a pipeline that was already red, where a new red is indistinguishable from the old
one**, and where nothing is a required status check ([H-23](#h-23)'s own correction). A gate nobody
reads is [H-45](#h-45) with a green tick instead of a missing target.

### The fix

`npm --workspace src/cleardeck_frontend run build:mainnet`, with **no** `DFX_NETWORK` in the step's
environment. That script is the one named mainnet path: it reads the ids from the tracked mapping
rather than being handed them, refuses a contradictory ambient `DFX_NETWORK` as a hard error, and
runs `verifyBundle` before exiting 0, deleting `dist` if verification fails. Run locally on this
tree: build succeeded, **8 of 8 bundle checks passed**, and the local dist was rebuilt afterwards
because both paths write to the same directory.

---

<a id="t-47"></a>
### T-47 — fund-theft — the fourth money door: a Bitcoin address fetched from an unpinned canister — STATUS: FIXED (wave 14)

Full write-up: **[FINDING 45](SECURITY-FINDINGS.md#finding-45)**.

Wave 14 closed three doors in `DepositModal.svelte` against a table id supplied by
`lobby.get_tables()` — an uncertified query. The derived ICP address, the OISY
transfer (`owner: tableCanisterId`) and the ICRC-2 approval (`spender:
tableCanisterId`) all refuse. The wave's own summary said *"all three money doors
refuse together"*, and the gate is named
`every_money_door_in_the_modal_refuses_an_unpinned_table`.

There were four.

```text
DepositModal.svelte:444   async function loadBtcDepositAddress() {
                            ...
                            const result = await tableActor.get_btc_deposit_address();

DepositModal.svelte:1394  {#if isBTC && depositMethod === 'btc'}
DepositModal.svelte:1417    <label>Your Bitcoin Deposit Address</label>
DepositModal.svelte:1419    <span class="address-text">{btcDepositAddress}</span>
DepositModal.svelte:1424    navigator.clipboard.writeText(btcDepositAddress)
```

`grep -n tableIsTrusted` over the file returns 51, 53, 378, 618, 619, 896, 1015,
1335 and 1574. **None of them encloses 1394–1440.**

### Why this is worse than FINDING 42, not a smaller version of it

FINDING 42 was a substituted argument to a hash the client computes. This address
is not computed at all — it is whatever the canister replied. That is
[FINDING 40](SECURITY-FINDINGS.md#finding-40), the defect FINDING 42 was the
sequel to, still unfixed on the BTC path, and Bitcoin sent to an attacker's
address cannot be reversed, refunded or clawed back by anybody.

The attacker also chooses the branch. `currency={getTableCurrency(currentTableInfo)}`
in `+page.svelte:1248` reads `Currency = variant { ICP; BTC }` out of the same
`get_tables()` reply, so a substituted registry naming a hostile canister with
`currency = variant { BTC }` selects the BTC flow and puts that canister's address
on screen with a Copy button. The fetch runs on mount.

### The fix and its gate

`loadBtcDepositAddress()` refuses before the fetch, and the render asserts the
trust root again next to the pixels, because this string is not derived by the
build from anything it pinned. Verified RED by removing both guards:

```text
loadBtcDepositAddress() asks an unpinned canister for a Bitcoin address
(guard at 6033, fetch at 1110) and the modal renders the reply under
"Your Bitcoin Deposit Address" with a Copy button. Bitcoin sent to it is
unrecoverable.
test result: FAILED. 6 passed; 1 failed
```

and green with them: `test result: ok. 7 passed; 0 failed`.

---

<a id="t-48"></a>
### T-48 — high — a tab that cannot read the table stopped asking the canister to advance its clock — STATUS: FIXED (wave 14)

`clockNudge.js`'s `decide()` computed, for a view it could not read:

```js
const seated = view === null
    ? this.lastCallAt !== null    // we have seen a table before; keep a pulse
    : ...
```

`lastCallAt` is set only in `observe()`, which runs only after a call is made. A
tab that has never obtained a decodable `TableView` has therefore never called, is
never `seated`, never reaches the 60-second backstop, and emits **nothing**:

```text
COLD-START null view, one day: 0 calls
```

`loadTableState()` leaves `tableState` null whenever `get_table_view()` throws or
returns `[]`; its catch block does not set it. The client this replaced called
`check_timeouts` as the **first** statement of the 500 ms poll, before
`get_table_view`, so it kept firing at 2/s in exactly that state.

### Why it is `high`

`check_timeouts` is what records `note_stall_opportunity`, and three of those
across the grace period is what makes `abandon_stuck_hand` reachable when the
on-chain timer is dead. A client that goes silent precisely when it cannot read
the table is [FINDING 15](SECURITY-FINDINGS.md#finding-15)'s fund lock rebuilt on
the client side — the one outcome `clockNudge.js`'s own header says it must not
produce.

The comment three lines above the defect said:

> a view that failed to decode reads as "no view" here, which is exactly when a
> backstop is wanted: "I could not tell" must not become "so I stopped calling".

**The comment stated the requirement and the code did the opposite.** The gate
beside it asserted `clockIsDue(null).due === false` and said *"the backstop covers
it"* without ever measuring whether the backstop fired.

### The fix and its gate

An unreadable view now counts as seated. Worst case is the backstop alone:

```text
COLD-START null view, one day: 1440 calls = 0.0099 T/day (budget 0.0200)
```

`node tools/shots/test-clock-nudge.mjs` → `a tab that never gets a readable view
still nudges the clock (T-48)`, verified RED by restoring `this.lastCallAt !== null`.

---

<a id="h-56"></a>
### H-56 — high — the gate that certified FINDING 42 measured text position, not control flow — STATUS: FIXED (wave 14)

`every_money_door_in_the_modal_refuses_an_unpinned_table` found the byte offset of
`!tableIsTrusted` inside `handleDeposit` and asserted it was lower than the offsets
of `icrc2_approve` and `wallet.transfer`. That is equally true of a guard that sets
an error message and falls through.

Deleting one token — the `return;` inside the guard — left the suite at **7 passed,
0 failed**, and left `node tools/shots/repro-finding42.mjs` at **exit 0**, printing
`✓ REFUSED`, while `handleDeposit()` ran on to
`ledgerActor.icrc2_approve({ spender: { owner: tableCanisterPrincipal } })` against
the substituted canister. The reproducer scrapes only the Claim button's `disabled`
(`repro-finding42.mjs:392`); nothing observed the Deposit button or the approval,
and nothing asserted any `disabled` binding at all.

### The repair needed two attempts, and the second one is the point

The obvious strengthening — *a `return` appears between the check and the money
call* — was written, and the same one-token mutation was re-run against it. **It
stayed green**, because the next `return` in that span belongs to the validation
immediately below:

```text
if (!tableIsTrusted) {
  error = untrustedReason;
                             <-- the deleted token
}
if (!depositAmount || Number(depositAmount) <= 0) {
  error = 'Please enter a valid amount';
  return;                    <-- this one satisfied the assertion
}
```

The assertion is now scoped to the guard's own brace-matched block, and the
disabled bindings are counted. Both verified RED against their mutations and green
on the tree.

---

<a id="h-57"></a>
### H-57 — high — the drain convicted money the canister pays on the very next call — STATUS: FIXED (wave 14)

Wave 14 replaced `check_drain`'s aggregate tolerance (`owed_after > 20_000`) with a
per-account rule at one ledger fee — the right change — and left `drain()`'s own
withdraw loop knocking at the old constant:

```rust
// tests/money_safety/src/invariants/reachability.rs
let bal = world.get_balance(*who);
if bal > 20_000 {                     // the drain's knock
```
```rust
// src/table_canister/src/lib.rs
let sweeping_whole_balance = amount == balance_now && amount > currency.transfer_fee();
```
```rust
// stranded_breakdown(), the conviction
if *amount > LEDGER_FEE_E8S {         // 10_000
```

So every escrow row in `(10_000, 20_000]` was convicted by an instrument that never
asked for it. Measured:

```text
the drain left 15000 e8s convicted as unreachable:
  [("escrow of 74yuz-2axoe-…", 15000)]
but the canister pays any whole balance above one ledger fee.
The drain transcript is:

  left: 15000
 right: 0
```

**The transcript is empty.** After the fix the same fixture reports nothing
stranded and the player's own ledger wallet is up exactly 5,000 e8s — 15,000 less
one fee — which is the assertion that keeps the green from being blindness.

### This is FINDING 31's dead band moving, not closing

`refund_external_deposit()` genuinely reaches everything above one fee at a deposit
subaccount, so the band is gone from the canister. It reappeared one layer out, in
the thing that measures the canister, where it produces a **false red** on M9 —
the top-severity invariant here — in `dev.sh test` and `fuzz-default`. This
repository keeps `artifacts/screens/acknowledged-reds.json` because reds get
skimmed; an invariant that cries wolf is the failure mode being defended against.

The threshold is now `LEDGER_FEE_E8S`, i.e. the canister's own rule rather than a
constant of the harness.

---

<a id="h-63"></a>
### H-63 — high — two gates on one directory, one green and one red, and CI ran the red one — STATUS: FIXED (wave 14)

Each `src/declarations/<n>/` holds three copies of one interface: `<n>.did`,
`<n>.did.js` and `<n>.did.d.ts`.

* `tools/gen-declarations` **writes** `.did.js` and `.did.d.ts`.
* `scripts/check-declarations-js.sh` **reads** `.did.js` and `.did.d.ts`.
* `scripts/check-candid.sh --declarations` **reads** `.did` — and it is the one
  wired into CI, at `.github/workflows/ci.yml:299`.

Nothing writes `.did`. When the lobby gained `refresh_table_config` and
`refresh_all_table_configs`, the two gates disagreed on the same directory in the
same tree:

```text
$ ./scripts/check-declarations-js.sh
  ✓ src/declarations/lobby matches src/lobby_canister/lobby_canister.did
  declarations: all 5 binding set(s) regenerate to exactly what is committed

$ ./scripts/check-candid.sh --declarations
::error::NEW declaration drift, not in the baseline:
      lobby	method-missing-from-a	refresh_all_table_configs
      lobby	method-missing-from-a	refresh_table_config
exit 1
```

### Why nobody caught it

The stream that added the lobby methods was reading the gate that cannot see
`.did`. The stream that saw the red read it correctly as *"another stream's
concurrent lobby change, not mine"* — true, and it meant neither owner was the one
who would fix it. Two correct local judgements, one red CI.

Fixed by hand-editing `src/declarations/lobby/lobby.did` (adding `Result_2` and the
two methods), which is what `check-candid.sh`'s own failure text instructs, because
that file carries hand-written documentation the extractor does not reproduce.
`--declarations` is exit 0 and the drift matches the 88-item baseline exactly.

**The durable version of this fix is not done:** the generator still does not write
`.did`, so the next method added to any canister lands in the same seam. It is
written down here rather than assumed.

---

<a id="h-58"></a>
### H-58 — high — nothing ties the shipped page to the clock policy — STATUS: OPEN

Every cycles number wave 14 published for the fixed client rests on `+page.svelte`
actually consulting `clockNudge.js`. Nothing enforces that link.

* `test-poll-updates.mjs` checks the timer's **period** (2000 ≥ the 2000 ms floor)
  and a shape budget (51,840 < 60,000).
* `test-clock-nudge.mjs` exercises the policy module in isolation and never reads
  `+page.svelte`.
* `test-burn-table.mjs` reads only JSON.

A `+page.svelte` whose `advanceTableClock` drops the policy and calls
`check_timeouts` unconditionally on its 2 s timer leaves **all three green**. That
is 43,200 calls/day/tab ≈ 0.298 T/day: 5.2x the published typical price and 1.54x
the published policy ceiling, which is the figure `fallback_burn_per_day` is built
from.

[T-48](#t-48) is the same seam from the other side — the policy itself was wrong,
and only a test of the policy could see it. Closing this needs a gate that resolves
`advanceTableClock`'s body and requires the decision to gate the call.

---

<a id="h-59"></a>
### H-59 — medium — the fix for H-45 blinded 70% of the gate built to catch H-45 — STATUS: FIXED (wave 14)

Check 4 of `check-suite-wiring.sh` — *"every target `scripts/dev.sh` names with
`--test` has a row, so the local gate cannot run something CI has never heard of"*
— discovered its subjects with:

```sh
named="$(grep -oE -- '--test [a-z_0-9]+' "$DEV_SH" | awk '{print $2}' | sort -u)"
```

Wave 14 replaced sixteen literal `cargo test --test X` lines in `cmd_test` with the
`run_ms X` helper. Those strings stopped existing:

```text
at HEAD:  23 subjects
after:     7 subjects
```

and it printed `ok` both times. **Only the number inside the message changed, and
nothing reads the number.** The sixteen it stopped watching were `admin_custody`,
`coherence_w8`, `cycles_runway`, `deposit_replay`, `deposit_subaccount_anchor`,
`deposit_surface`, `fund_reachability`, `invariants`, `ledger_boundary`,
`oldest_cluster`, `regressions`, `solvency`, `stall_agreement`, `timers`,
`ui_limits` and `wave6_coherence`.

With **zero** subjects it also still passed, because `printf '%s\n' "" | wc -l` is
1. And check 4 is the only one of the gate's five assertions with no `--selftest`
case: the selftest plants nine failures, covering checks 1, 2, 3 and 5.

Now it recognises both spellings and refuses an empty subject list:
`ok 25 target(s) named in dev.sh, all listed`.

---

<a id="h-60"></a>
### H-60 — medium — the new lobby leg prints a tick over zero comparisons — STATUS: OPEN

`check-deployed-config.sh`'s lobby-vs-contract loop:

```sh
IFS=',' read -ra pairs <<< "$tfields"
for p in "${pairs[@]}"; do ... done
[ -n "$bad" ] || echo "  ✓ lobby row $n (\"$name\") matches $cid"
```

If the python parse yields no fields for a row, `tfields` is empty, bash builds a
zero-length array, the body never executes, `bad` stays empty and the row is
reported as matching. That is [H-55](#h-55)'s shape — a green tick over an empty
comparison — in the same file, added by the fix for H-55.

The icp.yaml half of the same script guards it explicitly:

```sh
[ -n "$got" ] || { echo "  ! …could not read…"; fail=1; }
```

The lobby half has only a whole-registry `lobby_rows -eq 0` check, and `--selftest`
does not exercise the lobby half at all.

**Latent, not live.** Measured today against the running local lobby, the parser
extracts 8 of 8 fields from all three rows. It becomes live the first time the
record shape, a field name, or the CLI's Candid spacing changes — which is exactly
the event this script exists to survive.

---

<a id="h-61"></a>
### H-61 — medium — the burn table is verified only against itself — STATUS: OPEN

`test-burn-table.mjs` asserts that every scenario in `tools/cycles/burn-table.json`
recomputes from that file's **own** measured and inherited components. It never
compares any of them to the 13 raw runs in `artifacts/cycles/`. Its stated purpose
is *"a table whose rows do not follow from its own inputs is a table somebody
typed"* — and a table somebody typed **consistently** is what it lets through.

`scripts/cycles-runway.sh` then reads `FALLBACK_BURN_PER_DAY` straight out of it
with no verification, so the monitor's alarm floor can be moved silently.

Demonstrated: divide every non-legacy `per_tab_per_day` by 4 and recompute each
scenario with `build-burn-table.mjs`'s own formula. The gate exits 0 —
*"all 8 scenarios recompute from their own components"* — while the published
unknown-burn floor drops 2.0539 → 0.6075 T/day and the 51.4 T column goes from 25
days to 84, with the raw artifacts untouched.

This is the standing lesson of this repository in the cycles domain: the only
outside anchor (the runs on disk) is read by the builder and by nothing that gates.

---

<a id="h-62"></a>
### H-62 — medium — the "typical" per-tab price was measured with the nudger switched off — STATUS: OPEN

```text
$ python3 -c "...json.load(open('artifacts/cycles/fixed-1tab.json'))..."
  calls: {'heartbeat': 12, 'get_balance': 24, 'get_table_view': 240, 'get_shuffle_proof': 240}
  fixed-3tab:  {'heartbeat': 36,  ... }
  fixed-10tab: {'heartbeat': 120, ... }
```

**No `check_timeouts` key in any of the three cells.** The table used had 0 of 9
seats filled, and `clockIsDue` refuses both the predicate and the backstop on an
empty table, so the clock nudger — the entire mechanism being priced — fired zero
times in every cell that `fixed_poll_measured_idle_table` (0.0572 T/day) derives
from.

That price is then multiplied by 6 and 10 in the *"500 hands/day, N tabs open
(typical)"* rows, which are the rows [E-55](#e-55)'s corrected runway table and
wave 14's headline both quote. A table dealing 500 hands a day is by definition one
where the nudger fires between every hand, so the row is internally contradictory.

Measured on an occupied table: 3 `check_timeouts` per tab per 132 s ≈ 1,963/day
≈ 0.0135 T/day. The 10-tab typical row becomes ≈0.832 T/day (61 days on 51.4 T, 12
on 10 T) rather than 0.6974 T/day (73 / 14). The direction is safe-looking, and it
is the same species as [E-92](#e-92) itself: a per-tab price whose measurement
conditions excluded the thing being priced.

---

<a id="e-101"></a>
### E-101 — medium — one id excusing two opposite symptoms on one screen — STATUS: OPEN

[E-96](#e-96) says money the table cannot attribute *"renders NOTHING on the
deposit screen"*. `artifacts/screens/acknowledged-reds.json` cites **E-96** to
excuse `deposit/desktop` and `deposit/mobile` going red with:

```text
TOKEN CENSUS FAILED: 8 of 43 numeric tokens on screen are asserted by nothing
  287.93 / 28792955590 / 8   in section.solvency dl.figures dd
  "287.9296"                 in section.solvency > p.advice
```

— money figures that **do** render, one of them inside the `p.advice` sentence wave
14 rewrote. Both defects are real and they are opposites. A reader who follows the
acknowledgement to E-96 is told the screen shows nothing.

`SolvencyNotice.svelte`'s own comment states the requirement that was not met:

> NOTE FOR THE HARNESS. These are MONEY FIGURES … they need a site in
> `tools/shots/lib/chain-agreement.mjs`

Until the census red has its own id, its own site in `chain-agreement.mjs` and a
repointed acknowledgement, `./scripts/dev.sh shots` exits 1 with the deposit screen
UNVERIFIED at both viewports — where at `a65868e` both were VERIFIED with
*"0 unaccounted"*.


---

<a id="h-64"></a>
### H-64 — medium — the drain's stranded leg conserves against the canister's own books — STATUS: OPEN

```rust
// tests/money_safety/src/invariants/reachability.rs:554
escrow_after:          after.escrow.clone(),                        // admin_get_all_balances()
deposit_custody_after: after.canister_deposit_by_principal.clone(), // admin_deposit_custody()
```

```rust
// tests/money_safety/src/world.rs:94
/// The scanned set is the harness's actors, the controller, AND every
/// principal the canister itself names ...
pub ledger_deposit_by_principal: BTreeMap<Principal, u64>,   // icrc1_balance_of, per principal
/// **The CANISTER, per principal.** What it says is at each deposit address.
pub canister_deposit_by_principal: BTreeMap<Principal, u64>,
```

`stranded_breakdown()` reads the second and not the first. The ledger scan is in the
same struct, built for exactly this purpose, and its own comment says the point of
the pair is that they **can disagree**.

Wave 14's write-up states the principle correctly — *"recoverability is measured at
the player's wallet on the ledger, never at an escrow row, because an internal
figure cannot support the claim that money is reachable"* — and it is true of the
`deposit_floor` tests it was written about. It is not true of `check_drain`'s
stranded leg, and that is the leg that decides whether `dev.sh fuzz-default` is red.

### Why this is `medium` and not `high`

It is backstopped. `orphaned_e8s()` is anchored to `icrc1_balance_of`, and
`check_deposit_attribution` compares the canister's per-principal claims against the
ledger directly. So a canister that under-reported its own deposit custody would
still be convicted by a sibling leg today. What is missing is that **this** leg
cannot convict it, and the wave's own prose says it can.

### The fix

Iterate the union of `canister_deposit_by_principal` and
`ledger_deposit_by_principal` and take the MAX per principal. Max rather than the
ledger alone, because the canister may legitimately name a principal whose
subaccount the harness has not scanned, and the safe direction for a
*money-left-behind* check is to over-report.


---

<a id="h-65"></a>
### H-65 — medium — the deposit shot's fiat leg has a read-ordering race that produces a false red — STATUS: OPEN

```js
// tools/shots/lib/chain-agreement.mjs
// Re-read after the settle loop: a quote can land between the two.
const quote = servedIcpUsd();
if (dom.usdValues.length === 0) {
    if (quote && quote.mode !== 'unavailable') {
        structural.push(
            `a live quote (${quote.usd} USD/ICP) was served but the modal shows no fiat figure`,
        );
    }
}
```

The comment identifies the race exactly and the code only half-answers it: `quote`
is re-read after the settle loop, `dom` is not. If the third-party price lands in
that window, the harness ends up holding a fresh quote and a DOM snapshot taken
before the figure rendered, and convicts the product.

Observed once by the wave-14 coherence pass:

```text
! UNCOVERED PROBLEM on an acknowledged shot: deposit / desktop
    - DEPOSIT CHAIN DISAGREEMENT: a live quote (2.19 USD/ICP) was served
      but the modal shows no fiat figure
```

and **not reproduced** by the immediately following full sweep on the same tree,
the same replica and the same fixture — where `deposit/desktop`'s only problem was
the token census again.

### Why it is worth a row rather than a shrug

It lands on the one screen that already carries an acknowledged red
([E-101](#e-101)). The acknowledgement mechanism is per-problem: an entry lists the
substrings it `covers`, and any recorded problem outside them fails the gate. So an
intermittent extra problem on that shot turns `make hygiene` red for a reason that
is not real, on the shot a reader is already primed to skim — which is the exact
dynamic `acknowledged-reds.json` exists to prevent, arriving from the harness side.

The fix is to read the DOM's fiat values in the same breath as the quote, so the
two describe one instant.

---

<a id="e-102"></a>
### E-102 — high — two different engines are live on mainnet, and nothing could have said so — STATUS: OPEN

`table_1`, `table_2`, `table_3` and `btc_table_1` are four instances of one package,
`table_canister`. They differ only in `init_args`. Init args are install-time
arguments; they are not part of the module. So all four **must** report the same
module hash.

Read out of CERTIFIED state, 2026-08-09, `dfx canister info <id> --network ic
--identity anonymous`:

```text
table_1      4511ab187cff8b90202c4ba2c4e22aab4745636235aa4f3b2e9c69e79c42c608
table_2      9c0ed3a138a3753df5f415932e7822b88f625eaf5f81d5e2c9644dd45dfd537c
table_3      9c0ed3a138a3753df5f415932e7822b88f625eaf5f81d5e2c9644dd45dfd537c
btc_table_1  9c0ed3a138a3753df5f415932e7822b88f625eaf5f81d5e2c9644dd45dfd537c
```

`table_1` is alone. Whoever sits at whichever of these is older is playing with every
money defect that has since been closed on the other.

#### Why no gate could see it

There were two gates on deployed state and **both are per-canister**:

| gate | axis it checks |
|---|---|
| `scripts/check-deployed.sh` | each canister against an expectation |
| `scripts/check-deployed-config.sh` | each canister's live `TableConfig` against `icp.yaml` |

A fleet in which every canister is individually plausible, and which is collectively
incoherent, passes both. Nothing ever compared the canisters **to each other** —
which is the easiest invariant in this repository, because it needs no expectation at
all, only the observation that four things built from one source cannot differ.

`scripts/check-fleet-coherence.sh` (committed at `eb819a8`) is that missing axis. It
groups by package out of `icp.yaml` and fails if a group disagrees with itself.
Proven in both directions before it was believed:

```text
--network local   exit 0   all 3 package group(s) coherent
--network ic      exit 1   ✗ 2 DIFFERENT MODULES for one package
```

#### It also breaks the verification story

`README.md` tells a stranger to build the Docker image, read one sha256 per canister
and compare against what the IC reports. That image builds `table_canister` **once**,
so it prints one hash for the table modules. Against a split fleet, a verifier who
follows our own instructions exactly matches one canister of four and correctly
concludes the other three are not the code we published. The instructions are not
wrong; the deployment is.

#### How it happened, and what closes it

[E-79](#e-79): a deploy went out from a tree that agents were actively editing, and
`table_1` matched the tree while the rest did not. The lesson was written down and the
fleet was never brought back into line, so the split rode through every wave since.

Closing it is one operation, and the first step is the one that was skipped:

```bash
git status --porcelain                    # must be EMPTY. this is how it broke.
SKIP_CONFIRM=1 IDENTITY=cleardeck-prod ./scripts/deploy-mainnet.sh
./scripts/check-fleet-coherence.sh --network ic --expect HASHES.txt
```

`--mode upgrade` throughout; `reinstall` wipes balances and is what
[FINDING 23](SECURITY-FINDINGS.md#finding-23) is about.

---

<a id="e-103"></a>
### E-103 — high — the source we point verifiers at served a stale hash — STATUS: FIXED (wave 14)

[E-102](#e-102)'s gate was first built on the public dashboard API,
`https://ic-api.internetcomputer.org/api/v3/canisters/<id>`, for a good reason: it
needs no key, so any stranger can run it against our deployment without our
cooperation. That is exactly the property a verification instrument wants.

Within one hour the same endpoint returned two different module hashes for the same
canister, with **nothing deployed in between**:

```text
10:30   table_3   511c9d0e6d508d627357af22930b02daf252753621a17dd8a9fd0d5ab3796ba4
11:05   table_3   9c0ed3a138a3753df5f415932e7822b88f625eaf5f81d5e2c9644dd45dfd537c
```

Every deploy in that window was `-e local` — verified by scanning all five wave-14
builders' transcripts for mainnet commands, of which there were none. Certified state
says `9c0ed3a1…`. The dashboard is an INDEX over the IC. It lags, and it is not
certified.

**The first public reading of [E-102](#e-102) was wrong because of this** — reported
as three engines when the truth is two. That is the harmless direction. The other
direction is a verifier reading all-clear over a split fleet, or reading fraud into an
honest deployment, and there is no way to tell from the reply which one you got.

The fix is to read certified state. `dfx canister info <id> --network ic --identity
anonymous` reads the module hash out of the certified tree and still needs no key, so
the stranger property survives; the cost is one extra tool to install. The dashboard
path is **removed**, not demoted, so it cannot be reintroduced by someone
simplifying a `curl` back into place — the function carries the incident above it.

This is [FINDING 42](SECURITY-FINDINGS.md#finding-42)'s lesson — *the wire is not the
trust root* — arriving at the verification path one wave after it arrived at the
deposit path, and found by an instrument that was being written for a different
defect entirely.
