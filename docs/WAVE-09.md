# Wave 9: the reconciliation, and the seventh cross-agent defect

> **A NOTE ON THE NUMBER.** This is the ninth wave **document**. The register's `wave` column
> has run ahead of it: waves 9, 10 and 11 shipped without one being written, so the rows this
> document lands say `11`. The two counters are not the same counter and this file is the first
> place that says so. Where a row below says wave 11, it means the wave this document covers.

---

## THE ANSWER IS STILL NO.

A **fifth** independent auditor was run on the running system, given the repo as a stranger
finds it. The verdict, verbatim:

> **"NO — I would not tell a friend their money is safe in ClearDeck."**

> "Two things cost money, and they are different in kind.
>
> FIRST, a silent 100% loss at the number the product prints. The deposit modal says 'Minimum
> deposit: 0.0002 ICP' and, in the same modal, hands you a deposit address under 'Send ICP here
> from any wallet or exchange, then Claim'. Send exactly that minimum — 20,000 e8s — and
> claim_external_deposit sweeps it, pays the 10,000 e8s ledger fee out of it, and credits you
> 10,000. That 10,000 can never leave: withdrawal needs 20,000, and the whole-balance waiver
> needs strictly more than the 10,000 fee. It cannot be played either (min buy-in is 2 ICP). One
> e8 more would have been recoverable. The in-app approve route at the identical advertised
> minimum works fine, so the same number is safe on one route and total loss on the other. ckBTC
> is unaffected — only the ICP floor is set one unit too low.
>
> SECOND, custody is one key. I deposited 5 ICP as a player, then ran a single command as the
> controller — `icp canister install --mode reinstall`, same wasm, no code change — and the
> player's balance went to zero while the ledger still held the 5 ICP at the canister's account.
> Every player-callable recovery said she had nothing. The README discloses that a controller can
> destroy the hand history and the archive; it never says a controller can destroy your balance,
> while the front page sells 'fully decentralized' and 'fair play without requiring trust'."

The four before it, for comparison:

> **"NO — I would not tell a friend their money is safe here."** (fourth)
>
> **"No — I would not tell a friend their money is safe here."** (third)
>
> **"NO — I would not tell a friend their money is safe here."** (second)
>
> **"No. I would not tell a friend their money is safe here, and the reason is not the poker."**
> (first)

### The auditor's answer did not change, and neither of the two things it names is new

Both are already in the register, both have been there for waves, and **neither had an owner**:

| the auditor's finding | in the register as | filed | severity now |
|---|---|---|---|
| a deposit at the advertised minimum is a 100% silent loss | [FINDING 31](SECURITY-FINDINGS.md#finding-31) | wave 7 | **raised `high` → `critical`** |
| one controller key erases every balance | [FINDING 23](SECURITY-FINDINGS.md#finding-23) | wave 7 | **raised `critical` → `fund-theft`** |

This is the coherence pass's own sentence landing on the auditor's report: *"we have stopped
failing to FIND things and started failing to SCHEDULE them."* The severities are now what the
auditor measured rather than what the original filer guessed, so the two things that cost money
sort to the top of `./scripts/register-stats.sh`.

### And it named things that ARE new

Four, all filed this wave: [E-80](DEFECTS.md#e-80) (a `notify_deposit` refusal that asserts a
negative it cannot know), [E-81](DEFECTS.md#e-81) (a withdrawal refusal stating a universal
guarantee the same sentence disproves — shown to the player stranded by FINDING 31),
[E-82](DEFECTS.md#e-82), [E-83](DEFECTS.md#e-83). And one that is worth its own section.

---

## THE SEVENTH CROSS-AGENT DEFECT

Six consecutive waves have produced a defect with the signature **CORRECT TOTALS, WRONG
RECIPIENT, EVERY INVARIANT SILENT**. This wave was told to hunt the seventh deliberately and
first. It is [FINDING 43](SECURITY-FINDINGS.md#finding-43), and it is the purest instance so
far: **no e8 moves, nothing is destroyed, every number in the reply is arithmetically correct,
and the reply attributes a player's money to the house.**

The public solvency instrument calls the money at the shared main account a **surplus**, in the
same reply that names it as money held for somebody the canister cannot name.

```
cd tests/money_safety
cargo test --test deposit_surface -- --ignored --nocapture \
  the_solvency_verdict_counts_money_at_the_main_account_as_surplus
```

alice sends 1 ICP to the main account through the ledger's legacy `transfer` — the door
FINDING 34's published address sent every player to for eleven waves — and anybody calls the
public `refresh_solvency()`:

```
verdict              = CanPayEveryone
owed                 = 0
guard_liability      = 100000000
held                 = Some(100000000)
difference_e8s       = Some(100000000)
unattributed_at_main = Some(100000000)
summary              = This canister can pay everyone it owes ... it owes 0.0000 ICP (0 e8s)
                       and holds 1.0000 ICP (100000000 e8s), a surplus of 100000000 e8s.
```

**The mechanism.** `total_liability()` — what the currency guard reads — has five terms and the
fifth is `main_uncredited_observed()`. `get_solvency()`'s own `owed` has six terms and that is
not one of them, while `held` DOES include the main-account balance. The same e8 is counted as
an asset and never as a liability. The report publishes both totals side by side and computes
the verdict from the smaller one.

**Why it is a cross-agent defect.** Wave 10 added the fifth term to `total_liability()` because
[FINDING 35](SECURITY-FINDINGS.md#finding-35) showed a table sitting on 5 ICP reporting that it
owed nobody anything. The public `SolvencyReport` was written in the same wave and never got it.
Both agents were right about their own surface. The disagreement is invisible because each
surface is internally consistent, and `invariants::solvency` checks the report against itself.

**Why it matters.** This is what `DepositModal.svelte` renders before a player commits money.
And it is the same reading the fifth auditor got right after the controller `reinstall` wiped
5 ICP of a player's claim: `CanPayEveryone`, *"a surplus of 500000000 e8s"* — because wiping
`escrow_total()` lowers `owed`. **Money taken FROM a player improves this score.**

It is **not fixed** here, deliberately. `get_solvency` is another owner's surface,
[FINDING 38](SECURITY-FINDINGS.md#finding-38) is a second open defect in the same arithmetic,
and a wrong correction makes the instrument lie in the other direction. The marker is committed
`#[ignore]`d, red only when asked for, in the shape `dev.sh known-defects` already established.

---

## WHAT WAS RECONCILED

Four agents edited `src/table_canister/src/lib.rs`. All four changes are present, none undoes
another, and the wasm builds:

| change | present | interaction checked |
|---|---|---|
| `HandEnding` + `record_local_hand_result` + the narrowed recovery door ([FINDING 22](SECURITY-FINDINGS.md#finding-22), [E-78](DEFECTS.md#e-78), [E-79](DEFECTS.md#e-79)) | yes | `settle_unmovable_hand` is called INSIDE `TABLE.borrow_mut()`; it touches `HAND_HISTORY` / `CURRENT_ACTIONS` and `dispatch_hand_to_history` awaits before any state read, so there is no re-entrant borrow. Refunds go to the stake's OWNER (`apply_payouts`), and the stack sweep that follows credits each seat's own principal |
| `deposit_address_for` / `get_deposit_address` per caller ([FINDING 34](SECURITY-FINDINGS.md#finding-34)) | yes | every in-repo consumer re-checked; `coherence_w8` had already been moved off it. **One stale sentence found and filed as [E-80](DEFECTS.md#e-80)** |
| `DepositAddressCustody` + `canister` + `address` | yes | the hand-edited `.did` covers both new fields: `./scripts/check-candid.sh` is **0 / 0 / 0** structural differences against the built wasm |
| the hand-corrected `.did` files ([FINDING 03](SECURITY-FINDINGS.md#finding-03)) | yes | re-verified after the two field additions landed on top of them |

### One live defect was closed on the way

[FINDING 41](SECURITY-FINDINGS.md#finding-41) (`fund-theft`, raised by the wave-11 critic) was
still open: the OISY branch of `DepositModal.svelte` fetched its transfer destination from
`get_deposit_subaccount()` — the same uncertified query FINDING 40 is about, in its 32-byte
spelling — forty lines below the code that had stopped trusting it, and paid it automatically
with no address ever shown to the player. **Correct totals, wrong recipient, every invariant
silent**, in the shipping frontend, on the money path.

It now derives `depositSubaccount(sessionPrincipal)` locally and **aborts the transfer** if the
canister's reply disagrees, rather than warning about it — there is no player in the loop
between the check and the payment.

**The existing gate could not see it.** It compares the 64-hex form, which the OISY path never
touches, so it was green with the defect present: an instrument built by somebody who knew which
door they had repaired, testing that door. Two gates added, both in `deposit_surface`:
`the_frontends_32_byte_derivation_agrees_with_the_canister_for_every_principal`, and
`the_oisy_transfer_destination_is_derived_locally_and_not_fetched`, which reads the component,
finds the name the transfer is addressed to and requires it to be assigned from the local
derivation.

[FINDING 42](SECURITY-FINDINGS.md#finding-42) is the other half and is still **OPEN**: the
canister id the derivation is rooted in still arrives over `lobby.get_tables()`.

---

## THE GATES

| gate | result |
|---|---|
| `./scripts/dev.sh test` (workspace, wasm, differential, money-safety, settlement oracle, shots self-tests) | **green** |
| `make settlement` | **green** |
| `make fuzz-default` | **green** |
| `make known-defects` | **green** |
| `make hygiene` (includes `shots-verdict`) | **green** |
| `make selftest` (the mainnet guard) | **green** |
| `./scripts/check-candid.sh` | **green** — 0 / 0 / 0 structural differences |
| `./scripts/check-deployed-config.sh --network local` | **green** — all 4 table configs match `icp.yaml` |
| `./scripts/register-stats.sh --check` | **green** |
| `npx svelte-check` | 0 errors |
| the screenshot sweep, 24 scenes | **runs again** (see below); exits 1 on 10 acknowledged reds, and `shots-verdict` — the gate — passes |

### The notice check, on rendered pixels

**5 of 5 protected notices on screen in every one of the 24 scenes**, at both viewports:
the lobby, all six table scenes, behind the deposit modal, behind the hand-history and
hand-replay modals, behind the shuffle-proof panel, and — the dedicated scene — with a real
error toast raised by the app's own error path. Every scene also records
*"notices survive an error toast"* independently.

### The sweep could not run at all until this wave diagnosed why

The first attempt failed **all 24 scenes** with *"Lobby has no registered name for table_N"*
while `./scripts/dev.sh local-up` printed `✓ lobby lists 0 table record(s)` and exited **0**.
That is [E-74](DEFECTS.md#e-74), and the root cause is now written down: `set_admin` is called
as `$CONTROLLER`, the lobby's admin is whoever initialised it first, and when those differ both
that call and `init_microstakes_tables` are refused — silently, because `icp canister call`
exits 0 on `variant { Err }` ([E-50](DEFECTS.md#e-50)). `up_wire` now reads the count back and
**dies**, printing `get_admin` and the hand-over command. E-74 is raised `medium` → `high`: it
takes the project's only rendered-pixel gate offline without a word, and it had.

The 10 reds are all pre-existing and all acknowledged: [E-65](DEFECTS.md#e-65) (the lobby's
stored stakes are 5x/10x the tables' real ones), [E-71](DEFECTS.md#e-71) (the archive's
`(table_id, hand_number)` is not a key), [E-75](DEFECTS.md#e-75) (9.8% of a stack figure covered
by the pod clock at 390x844), [E-76](DEFECTS.md#e-76) (the felt below its floor on three shots).

### Byte-reproducibility

`./scripts/verify-build.sh --two-paths` is container-only and **Docker is not available in this
environment**, so the property was measured on the host instead: the same source built at
`/p1` and at `/a/deliberately/much/longer/absolute/path/for/the/very/same/source`.

* With the **artifact pipeline's own flags** (`RUSTFLAGS="-Cstrip=symbols
  --remap-path-prefix=…"`, exactly what `recipes/rust-reproducible.hbs` sets):
  **identical, all three modules.**
* With a bare `cargo build`, `table_canister` **differs** — and the difference is entirely the
  debug `name` custom section (319,927 vs 319,946 bytes). Every semantic section is byte-equal,
  including the 2,315,929-byte code section and the 188,259-byte data section. That is
  [H-29](DEFECTS.md#h-29) reproduced and localised, and it is the exact mechanism the recipe's
  own header names.

**The deliverable artifact is path-independent. The raw `cargo build` output is not, and never
was the artifact.** The container check still has to be run by whoever has Docker before the
redeploy.

⚠️ **Mainnet will not match this tree until the lead redeploys.** Correcting the `.did` files
([FINDING 03](SECURITY-FINDINGS.md#finding-03)) moved every module hash, because the `.did` is a
build input that ships as on-chain `candid:service` metadata. And until this wave the redeploy
**could not happen at all**: see [T-41](DEFECTS.md#t-41).

## THE REGISTER

`./scripts/register-stats.sh --check` passes: closed vocabulary, every `FIXED` names a gate,
every anchor resolves, every entry counted.

Nine `FIXED` rows still name a gate that **no target runs** ([H-45](DEFECTS.md#h-45)). That
list is the standing risk in this register: a fix held by a gate nothing invokes is a fix
nobody will notice regressing.

---

## WHAT TO DO NEXT, WORST FIRST

1. **[FINDING 31](SECURITY-FINDINGS.md#finding-31)** — the deposit floor. One constant, and the
   invariant has to be restated on `min_deposit − sweep_fee ≥ min_withdrawal` rather than on
   `min_deposit ≥ min_withdrawal`, which is true and beside the point. It is the cheapest
   fund-loss fix in the register and it is the number on the screen.
2. **[FINDING 23](SECURITY-FINDINGS.md#finding-23)** — custody is one key, and the player-facing
   text does not say so. Two separate jobs: disclose it where a depositor reads, and reduce it
   (blackhole, NNS/SNS control, or a threshold).
3. **[FINDING 43](SECURITY-FINDINGS.md#finding-43)** — the solvency instrument, before anyone
   is pointed at it again as evidence their funds are backed.
4. **[FINDING 42](SECURITY-FINDINGS.md#finding-42)**, then [FINDING 39](SECURITY-FINDINGS.md#finding-39).
