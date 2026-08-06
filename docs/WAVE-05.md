# Wave 5: what is proven, what is not, and what a stranger found

Five agents built. One independent auditor, given the running local stack and the public repository
and told to treat every document in it as marketing, tried to break the product. This pass
reconciled the frontend across four concurrent editors, walked the whole app at both viewports,
re-measured every headline claim on the rendered page, triaged the auditor into
[DEFECTS.md](DEFECTS.md) and [SECURITY-FINDINGS.md](SECURITY-FINDINGS.md), and ran the gates.

**The one-sentence verdict.** Wave 5's frontend work is real and it is now measured by the harness
rather than asserted in prose: the phone's playing surface is 60.6% of the frame with all five
protected phrases on screen, and both of those facts are assertions now. But the wave shipped with
**HARD RULE 2 live-broken on the whole desktop app**, and the auditor found something worse than
anything in the UI: **a funded table can be locked with every path to a player's money closed at
once, and nobody can check what code is running.**

---

## 1. The auditor's verdict, quoted

Not paraphrased, because the paraphrase is where the comfort creeps in.

> No. I would not tell a friend their money is safe here, and the reason is not the poker.
>
> The fairness claim is the strongest thing in the project and it holds. I wrote a verifier from
> `docs/SHUFFLE-SPEC.md` alone, without reading or running any of the project's own verifiers, and
> it reproduced three hands from the running canister exactly: every hole card, every board card,
> including a hand where I recorded the commitment while only the flop existed and then predicted
> the turn and river, and including a folded seat's cards the canister never published. The spec is
> precise enough that I had to guess nothing. Two honest caveats on the wording: "the commitment is
> published before the deal" is only true inside a single message, since `start_new_hand` commits
> and deals atomically and no outsider can observe the commitment before cards exist (what is
> genuinely provable, and what I did prove, is that the whole 52-card order was fixed before the
> board was shown); and the seed's unpredictability rests on trusting the subnet's `raw_rand`, which
> the spec says plainly.
>
> What fails is everything around it. First, I cannot tell what code is running. The deployed module
> hash matches no commit in the repository, and the build is not even path-independent: the same
> source at two different directories yields two different hashes because a 274 KB debug-name
> section survives the shrink step. The published Docker verification cannot compile at all, because
> its COPY list omits the `poker_core` crate that the table canister depends on. Second, this is not
> academic. While playing a normal hand I bricked a funded table: one player stopped heartbeating for
> thirty seconds pre-flop and every state-advancing call began trapping in the hand evaluator, while
> `withdraw` and `cash_out` both refused with "Cannot withdraw while in a hand". About 420 ICP was
> unreachable through every path a player has. I then replayed that exact state against the current
> source in my own host harness and it settled correctly. So the software I was invited to inspect
> and the software actually holding money are different, and the one holding money is worse. That is
> the finding that matters: a verifiable shuffle inside an unverifiable binary buys a player very
> little.
>
> Third, the fairness record is not durable. The history canister is deployed, authorised for no
> tables and empty; proofs live only in the table, capped at 100 hands and prunable, and one admin
> key can reset them. Fourth, the endpoint a player would actually reach for, `verify_shuffle`,
> returns false for a valid proof if you pass the seed and hash in the natural order, with no
> parameter names in the Candid and a bool return, so "you called it backwards" and "you were
> cheated" are indistinguishable. Fifth, a player who stops heartbeating is skipped by the betting
> logic while staying eligible for the pot, which is a client-controlled free run to showdown at
> everyone else's expense.
>
> What I would fix first, in order: make the deployed artifact identifiable (strip the name section
> or set a stable build path, embed the git revision as canister metadata, fix the Dockerfile's COPY
> list and pin the base image, and read the hash over the public read-state path instead of a
> controller-only status call) — until that is true nothing else in the repository can be trusted to
> describe the running system. Then stop treating a missing heartbeat as an absence of obligation in
> the betting state machine, and make every settlement evaluator call refuse rather than trap. On
> the credit side: no rake, chip totals conserved to the e8 in every hand I settled, `withdraw` is
> properly guarded against reentrancy, the admin queries reject non-controllers, mucked cards stay
> hidden, and the dev faucet really is dead. The README's own warning that funds are not safe is, as
> far as I can tell, the most accurate sentence in the documentation.

Its own answer to "what would you fix first" is worth keeping separate, because it is a gap in our
gates and not in the code:

> Settlement entitlement, as opposed to chip conservation. I verified that chips are conserved to the
> e8 in two settled hands and that a four-way side-pot payout matched the eligibility blobs, but I
> did not build an independent oracle for who is OWED what across multi-way all-ins, short all-ins,
> mid-hand departures and split pots with odd chips. Conservation is the invariant that cannot see
> the interesting bug: money can be perfectly conserved while being paid to the wrong player.

**Triage:** all eleven findings are in [DEFECTS.md](DEFECTS.md#found-by-the-wave-5-coherence-pass)
as E-42 to E-48, T-32 to T-36, D-06 and H-39/H-41. The fund lock is written up in full as
[SECURITY-FINDINGS FINDING 15](SECURITY-FINDINGS.md). Nothing was explained away, and where the
auditor could not verify something we believe is true, it is filed as a defect of
**discoverability**, which has a different fix.

One point of correction, in our favour and worth stating precisely because it changes the fix: the
auditor's own settlement-entitlement gap is **already closed for the hands the oracle runs**.
`tests/settlement` is an independent oracle that asks who is OWED what, by principal, and
[H-12](DEFECTS.md#h-12) records it. What is true is the auditor's narrower point: **it does not run
against the deployed binary**, and no CI job runs it at all ([H-23](DEFECTS.md#h-23)).

---

## 2. What this pass changed

Small, and all of it forced by measurement.

| file | change | why |
|---|---|---|
| `routes/+page.svelte` | the canonical no-rake sentence added to `.banner-info` and `.disclaimer-info` | HARD RULE 2 was broken on every desktop view except Hand History, and in the portrait FULL TERMS sheet ([T-36](DEFECTS.md#t-36)) |
| `DepositModal.svelte`, `WithdrawModal.svelte`, `HowItWorks.svelte` | same sentence added on its own line | the modals restated the property in weaker words, leaving the protected sentence behind the scrim |
| `tools/shots/run.mjs` | `probeProtectedNotices` + a new felt gate run centrally | the notice probe existed and was wired to two scenarios ([H-40](DEFECTS.md#h-40)) |
| `tools/shots/lib/felt-area.mjs` (new) | measures and floors the playing surface | there was no felt assertion anywhere in the harness |
| `tools/shots/lib/capture.mjs` | `NOTICES` and `felt` columns in `INDEX.md` | so a reader sees both halves of the rule in the index |
| `tools/shots/lib/chain-agreement.mjs`, `lib/token-census.mjs`, `token-allowlist.mjs` | assert the deposit modal's third figure; allowlist "18+" inside in-dialog notices | the deposit scene was UNVERIFIED at both viewports ([H-41](DEFECTS.md#h-41)) |

**Every notice edit is purely additive.** No existing notice line was changed, which is checkable
and was checked:

```
$ git diff fe72d46 -- README.md src/cleardeck_frontend/src | grep '^-' \
    | grep -Ei 'unaudited|18\+|jurisdiction|rake'
(no output)
```

**No engine source was touched.** `git diff fe72d46 -- src/table_canister src/poker_core
src/lobby_canister src/history_canister` is empty, including after the auditor's critical finding , 
deliberately, because HARD RULE 3 says a fund finding is written up and never quietly patched, and
because E-42 needs a failing test before it needs a fix.

---

## 3. HARD RULE 2, measured on the rendered page

The rule the repository has already broken once. Measured with the repo's own gate
(`tools/shots/lib/protected-notices.mjs`, `elementFromPoint` on each phrase's own pixels, ≥60% of
sampled points must land on the phrase), driving the real app against the real local canisters, on
the dist built from the tree as the four wave-5 agents left it, and again after the fix.

| surface | 1440×900 before | after | 390×844 before | after |
|---|---|---|---|---|
| lobby, signed out | 4/5 | **5/5** | 4/5 | **5/5** |
| lobby + How it works | 4/5 | **5/5** | 4/5 | **5/5** |
| lobby, signed in | 4/5 | **5/5** | 4/5 | **5/5** |
| table, as it lands | 4/5 | **5/5** | 5/5 | 5/5 |
| table + FULL TERMS open | n/a | n/a | 4/5 | **5/5** |
| table + Deposit modal | 4/5 | **5/5** | 4/5 | **5/5** |
| table + Hand History | 5/5 | 5/5 | 5/5 | 5/5 |
| table + Verify Fair | 4/5 | **5/5** | 5/5 | 5/5 |

The missing phrase was *"No rake is taken from any pot on any table"* in every single case. Two of
those rows are worse than an omission and are worth naming:

- **The FULL TERMS sheet was not a superset of the strip it covers.** Tapping "FULL TERMS" to read
  the terms took the no-rake sentence off the screen.
- **The money modals restated the property as "0% rake"**, so the protected sentence stayed outside
  the dialog, behind the scrim the in-dialog notice exists to escape.

**Withdraw could not be measured and is now known to be untestable here.** The Withdraw button is
`disabled={tableBalance <= 0}` and the walk's hero holds chips at the table, not escrow, so the
dialog never opens; the source change is identical to Deposit's and is asserted by `make hygiene`'s
source check only. That is a gap, recorded as such.

### The one-line sabotage, and what happens now

The pass's critic proved the hole by building it: flip `.banner-strip { display: block }` to
`display: none` and a phone player sees **zero of five** protected phrases on the table view, the
felt rises to 61.6%, and every gate in the repo stays green. Re-run on the shipping build with the
gate wired in, mutating one declaration in the shipped stylesheet and serving it through the
harness's A/B path (`SHOTS_SERVE_DIST`, chain traffic untouched):

```
▸ table-preflop [mobile]
  ⚠ NOTICE OFF SCREEN: protected notice "Unaudited code with known bugs" is NOT on screen …
  ⚠ NOTICE OFF SCREEN: protected notice "your funds are NOT safe" is NOT on screen …
  ⚠ NOTICE OFF SCREEN: protected notice "illegal in many jurisdictions" is NOT on screen …
  ⚠ NOTICE OFF SCREEN: protected notice "18+ only" is NOT on screen …
  ⚠ NOTICE OFF SCREEN: protected notice "No rake is taken from any pot on any table" is NOT …
  ✗ STATE NOT VERIFIED
▸ table-showdown [mobile]   ✗ STATE NOT VERIFIED  (same five)
```

And the mirror mutation, because a notice gate on its own is an incentive to shrink the felt
instead. Changing `--fw: min(86cqw, 55cqh)` to `min(50cqw, 32cqh)` in the shipped stylesheet, with
the notices left alone:

```
▸ table-preflop [mobile]
  ▪ felt 193.6x348.8 = 20.5% of frame, aspect 0.555, 6 pods (default), floor 45%
  ⚠ FELT: the playing surface is 20.5% of the 390x844 frame … below the 45% floor for mobile
  ✗ STATE NOT VERIFIED
```

20.5% is wave 4's regression, near enough. Both halves of the rule are now gated, in both
directions, centrally, for every scene at every viewport.

---

## 4. The felt: what the numbers actually are

Measured on `.felt`, the layout box of the visible green surface. **Measuring `.poker-table`
instead reads 1400×629 = 68% of a 1440×900 window where the surface is 31.7%**: a factor of two,
and this pass's own first probe fell into it.

| scene family | table | 1440×900 | 390×844 |
|---|---|---|---|
| `table-preflop` / `table-facing-bet` / `table-showdown` | `table_2`, 6-max | 929.3×442.5 = **31.7%** area (64.5% of window width), aspect 2.10 | 332.8×599.5 = **60.6%** area, aspect 0.555 |
| `table-empty` / `table-allin` / `table-sidepots` | `table_3`, 9-max | 950.3×452.5 = **33.2%** (66.0% width) | 304.2×548.1 = **50.7%** |

Reproduces exactly across three separate runs and two independent probes.

**The 18 px the notices cost.** Putting the canonical sentence where a desktop player can see it
adds a line to `.banner-info` at 1440×900, which takes 18 px of stage height: the desktop 6-max felt
was 961.1×457.7 and is now 929.3×442.5. Bar 1 (70% ±8 of window width) is still cleared, at 64.5%
instead of 66.8%. Portrait is unchanged, because the strip already carried the sentence there.
Recorded in [DESIGN-BAR §10.3](DESIGN-BAR.md) so nobody has to rediscover it. The place to buy the
18 px back is the non-protected marketing clause in the same paragraph, never a notice.

**60.6% is the SETTLED state, and that was never said.** `PokerTable.svelte:313` falls back to
`max_players ?? 9`, so **every table first paints a nine-seat ring** and corrects when the canister
answers. Sampled per animation frame on entry to a 6-max table:

| viewport | first paint | held for | settles to |
|---|---|---|---|
| 390×844 | 287.5×518.1 = **45.3%** → 50.7%, 9 pods | **336 ms** | 332.8×599.5 = **60.6%**, 6 pods |
| 1440×900 | 950.3×452.5 = **33.2%**, 9 pods | **304 ms** | 929.3×442.5 = **31.7%**, 6 pods |

A visible 9.4% linear jump of the whole table, on every phone entry. Pre-existing (`?? 9` is
byte-identical at `fe72d46`); made conspicuous by wave 5, because 6-max and 9-max portrait now
resolve from different formulas. Filed as [T-32](DEFECTS.md#t-32), and it is why the new gate records
the pod count and ring class beside every felt number: a felt figure without them does not say which
of the two states it measured.

---

## 5. Scene-by-scene A/B standing

One convention throughout: a row is scored only where a **real-gameplay** reference capture exists.
Marketing creatives are recorded and excluded, which is the corpus's own instruction.

**Provenance, so a reader knows what this pass re-measured.** The **ClearDeck side of the two table
rows** was re-measured here from scratch, three times, and is now a harness assertion: 60.6% / 50.7%
portrait, 31.7% / 33.2% desktop, aspect 2.10 / 0.555. Everything else, the competitor figures, the
lobby rows, and the replayer's field counts, is **carried forward from the builders' own reports
after their critics had checked it**, and is not independently re-measured in this pass. Where a
critic overturned a number, the corrected number is the one in the table and the correction is listed
below it.

| scene | competitor | verdict | the number |
|---|---|---|---|
| portrait 6-max table, 390×844, notices on screen | PokerNow `mobile-portrait-1.png` (`real_gameplay=true`) | **ours** | 60.6% of frame against 49.1%. 3.3× wave 4's shipped 18.3%, 2.9× the 20.9% geometry it replaced |
| portrait 9-max table, 390×844 | PokerNow, same capture | **tie** | 50.7% against 49.1%, level within the detector's spread, and the hard ceiling: the 9-max felt is width-capped at `min(78cqw, 52cqh)`, confirmed empirically by a build with the notice strip removed entirely (unchanged at 304.2×548.1) |
| portrait table vs app-store creatives | PokerStars 27.4%, GGPoker 40.3% (fill SUSPECT), WPT 0.2% (detector MISS) | **excluded** | `INDEX.json` says `real_gameplay=false` means proportions are not authoritative |
| desktop table geometry | reference median | **ours, in band** | aspect 2.10 (bar: 1.9–2.3), width 64.5–66.0% of window (bar: 62–78%) |
| portrait 9-max pot readout legibility | PokerNow portrait | **theirs** | 22.0% of `.equity-method` covered on `table-allin`, 14.4% on `table-sidepots`, 23.8% of `.pot-breakdown` on the 6-max scenes. Pre-existing and proportionally unchanged this wave, and **under the pixel gate's own threshold, so no gate will ever report it** |
| lobby, desktop 1440×900, first-choice position | PokerStars desktop lobby | **tie** | ours 34.8% against their 34.4%; they win density (24 rows × 9 fields to our 3 × 7), we win disclosure and live-truth-signed-out |
| lobby, desktop, master/detail depth | WPT Global | **ours** | ~19 distinct facts in the preview against ~4, under one counting convention applied to both |
| lobby, phone 390×844 | PokerStars mobile | **theirs** | first card at 57.4% against ~20%; 15 points outside the band's 42.6% ceiling. Structural: 387.5 px of banner + header is 45.9% of the screen before `Lobby.svelte` paints |
| lobby, signed out, is there anything to look at | GGPoker | **ours** | three real tables with live pot, phase, seat occupancy and hands dealt, against an 18+ interstitial over a promo carousel and zero live tables. One axis goes the other way: their 18+ check is a blocking modal, ours is a line of text |
| hand replayer's action log | PokerNow Session Log (`real_gameplay=true`) | **ours** | 5 rendered fields per action line against 3; amount on 9 of 10 lines (100% of chip-moving lines) against 6 of 13; plus a self-audit line that reconciles to the e8 and a provenance sentence naming the wire fields. They win on stack context |
| hand replayer vs GGPoker PokerCraft | n/a | **not judgeable** | the corpus contains no PokerCraft action log; the three "hand-history" entries are the EV Graph tab and a marketing composite |

**Corrections to A/B rows claimed by builders this wave:**

- The mobile pass reported the win as coming "from chrome, NOT from the notices". Its own critic
  built the honest alternative and measured it: the full-text notices in the flow give
  218.3×393.4 = **26.1%**, so **34.5 of the 42.3-point 6-max gain comes from compressing the
  protected block** from 268 px of 12 px text to 60 px of 11 px text, and 7.8 points from the chrome
  work. The compression is legitimate (every protected phrase is verbatim and on screen, one tap
  from the full text) but "from chrome" describes 18% of the win.
- The mobile pass's T-25 evidence table records wave 4 as "5 of 5" notices on screen. It was
  **4 of 5**: the repo's own gate reports the canonical no-rake sentence with zero carriers on every
  wave-4 mobile table scene, and the same report's prose says so two paragraphs earlier.
- The pixel-gate pass's desktop half of T-23 is **not reliably reproducible**: the winning seat is
  re-dealt every run, and the defect is only detectable when a flank seat happens to win.

---

## 6. Reconciliation: four editors, two of them inside `PokerTable.svelte`

Driven, both viewports, in one session, on the real canisters. All of it present at once:

| owner | thing | present |
|---|---|---|
| pixel-overlap gate | `.equity-badge` moved out of `.player-nameplate`'s stacking context onto the seat's readout spoke | yes, `--rdx/--rdy` resolve per seat and per orientation (`-1,0` desktop, `0,1` portrait), 0 occluded on all 22 shots |
| pixel-overlap gate | `.winner-award` given its own per-seat vector | yes, `--ax/--ay` resolve (`0.182,-0.065` desktop, `0,-0.2856` portrait) |
| pixel-overlap gate | `.equity-method` as one snippet rendered into whichever readout is on screen | yes, on screen at showdown at both viewports (`Equity · exact · 1 runout`) |
| mobile vertical budget | `initial-scale=1`, two-row portrait header, 60 px notice strip | yes, `documentElement.scrollWidth == innerWidth` at both viewports, header 69.0 px portrait / 82.5 px desktop |
| mobile vertical budget | portrait felt geometry | yes, 60.6% / 50.7%, now asserted |
| lobby last mile | container-query fix for the clipped `Sit`/`View`/`Watch` control | yes |
| lobby last mile | How-it-works dialog above the banner | yes, close button hit-tests to itself at both viewports |
| hand replayer | `phase` + `amount` on every action, the self-audit line, in-dialog notices | yes, `handreplay` verified at both viewports |
| money copy | in-dialog notices in both money modals, T-26/T-28/T-29/T-30 | yes for Deposit; **Withdraw is source-verified only** (button disabled at zero table balance) |

No conflict was found between the two `PokerTable.svelte` editors. Zero console errors, zero page
errors, zero failed requests across the whole walk at both viewports.

---

## 7. The walk: every seam a new user hits

Lobby → How it works → sign in → sit → play to showdown → hand history → verify the shuffle →
deposit. Both viewports.

1. **The lobby misprices every table it lists.** The row labelled `6-Max - 0.01/0.02` is a table
   charging 0.05/0.10, and `9-Max - 0.01/0.02` charges 0.10/0.20, **five times and ten times the
   blind the player chose.** This is the whole reason both lobby scenes are red, and the harness
   says it on every run. Re-severitised from "stale registry" to **high**; see the correction to
   [L-04](DEFECTS.md#l-04). The previous pass filed it as needing a lobby method that does not
   exist; verified read-only here, `is_caller_admin` returns `true` for the local identity
   `cyclepay-hotwallet` and `init_microstakes_tables` already rewrites every registered record, so
   the durable fix is six constants and a lobby redeploy.
2. **The table resizes under you on entry** ([T-32](DEFECTS.md#t-32)).
3. **The Withdraw button cannot be opened** in the state a player is in immediately after sitting
   down (chips at the table, zero escrow), so its notices and its money copy have never been seen on
   a rendered page by anyone.
4. **No BTC surface exists locally at all** ([T-35](DEFECTS.md#t-35)), so the corrected 11-sat
   withdrawal minimum, the only figure in the app that ever carried a 90.9× error, has been
   verified against the canister and in the source and **never on a screen**.
5. **`verify_shuffle` answers "false" to an honest question** asked in the natural argument order
   ([E-44](DEFECTS.md#e-44)).
6. **The hand history you verified today may not exist tomorrow** ([T-34](DEFECTS.md#t-34)): the
   history canister is empty and authorised for nothing, the table keeps 100 hands, and one admin
   call wipes them.
7. **The equity-method line is clipped at both ends** by the flank seat pods on the portrait 9-max
   readout, pre-existing, unchanged this wave, and below the pixel gate's threshold, so no gate
   will ever report it.

---

## 8. Gates

Run on this tree, in this pass. Engine sources byte-identical to `fe72d46` throughout.

| gate | result |
|---|---|
| `git diff fe72d46 -- src/table_canister src/poker_core src/lobby_canister src/history_canister` | **empty**: no engine source changed |
| `./scripts/dev.sh selftest` | **PASS**: every hostile mainnet argument shape refused, 7 ids in the denylist |
| `./scripts/dev.sh known-defects` | **1 of 1 engine defects still present** (`defect_detect_straight_returns_the_best_straight`), unchanged |
| `./scripts/dev.sh hygiene` | **every notice check green; the size rule RED after a shots run.** Measured both ways in this pass, see below. Do not read a single verdict off this row |
| `./scripts/dev.sh shots` | **20 of 22 shots verified**; 1 scene red (`lobby`, both viewports) for the L-04 mispricing and nothing else. Full numbers below |
| `npm run selftest` (tools/shots) | **PASS**: `ALL 16 PIXEL-GATE CASES PASS`, `ALL 16 CENSUS CASES PASS` (17 allowlist rules), `ALL PARSER CASES PASS` |
| `make settlement` | **PASS**: `9 passed; 0 failed` + `3 passed; 0 failed`, **51 hands settled**, 0 disagreements with the oracle, 0 chips collected and paid to nobody, 0 chips paid to a seat not owed them. Table wasm `sha256 5996594741afd63f88f38098c5461a24fbacafbda16278fc44bd3317a6c55c23`, identical to `./scripts/dev.sh doctor` |
| `./scripts/dev.sh test` | **the wrapper hung; every leg inside it is green when run directly.** See below. Filed as [H-42](DEFECTS.md#h-42) |
| - `cargo test --workspace` | **PASS**: 114 tests across 12 targets, 0 failed |
| - `tools/differential` fast subset | **PASS**: 22 passed, 2 ignored, 0 failed |
| - money-safety `invariants` | **PASS**: **45 passed, 0 failed**, 63.68 s at `--test-threads=1` (the same target hung for 33 minutes at `--test-threads=2`) |
| - money-safety `regressions` | **PASS**: 6 passed, 0 failed |
| - money-safety `deposit_replay` (the E-02 fund-theft reproducer) | **PASS**: 9 passed, 1 ignored, 0 failed |
| - money-safety `ui_limits` (T-26's gate) | **PASS**: 6 passed, 0 failed |
| `./scripts/dev.sh fuzz-default` (H-28's gate) | **PASS**: 1 test, 43.69 s, 15 hands completed, M8 principal attribution on 13 of them, 10 also checked against the independent settlement oracle |
| seam mutations ([H-04](DEFECTS.md#h-04)) | **6 of 7, unchanged**. Asserted, not re-run: the engine is byte-identical and the only harness change is additive |

### The definitive screenshot run, read out of its own manifest

```
11 scenes, 22 shots
540 money/equity/card figures on screen
2,895 (figure, occluder) pairs pixel-tested
0 occluded
22 of 22 shots at 5/5 protected notices on screen
0 shots below 5/5
20 shots carry a felt measurement; the floor is ASSERTED on 12 of them
1 scene not verified: lobby
```

Read the felt line precisely, because it is the one number in this document that overstates itself
if skimmed. The floor is asserted on the **twelve plain table views**: `table-empty`,
`table-preflop`, `table-facing-bet`, `table-allin`, `table-showdown`, `table-sidepots` at both
viewports. On the other eight (`deposit`, `handhistory`, `handreplay`, `shuffleproof`) a dialog or
the fairness side panel legally reflows or covers the table, `shuffleproof` desktop is 24.8%
**because** the proof panel is beside the felt, so the geometry is recorded and the floor is not
applied, and `lib/felt-area.mjs` decides that by looking at the page (is a dialog or panel on
screen?) rather than at the scene's name, so a scene added tomorrow cannot escape it by being
called something else.

One caution about reading those artifacts, learned by breaking them: **a partial run
(`--scenes lobby`) overwrites `INDEX.md` and `manifest.json` with only the scenes it ran**, in both
`<sha>/` and `latest/`. The PNGs survive; the record of what they prove does not, and nothing warns.
Filed as [H-43](DEFECTS.md#h-43). The numbers above come from a full run.

One notice row needed the scroll-top fallback and is recorded as such: `shuffleproof` mobile scrolls
the proof panel into shot, which puts the banner at y = −90. At the top of the view it is 5/5. The
gate evaluates the rule at the top of the view because that is what the rule is about, wave 4's
crime was notices 248 px of scroll below the fold, and records both measurements.

### The primary gate hung, and it has no time bound

`./scripts/dev.sh test` got through 22 `test result:` lines, entered `cargo test --test invariants --
--test-threads=2` in `tests/money_safety`, and stopped there for 33 minutes with **zero CPU on both
the test binary and its own PocketIC server** and a live socket between them. That is a blocked wait,
not a slow one. Four PocketIC servers belonging to concurrent agents were running on the machine, so
it may not be deterministic, which is worse rather than better.

`cmd_test` wraps the **settlement** targets in `with_timeout 900` / `with_timeout 300` and wraps the
**money-safety** targets in nothing. That is [H-22](DEFECTS.md#h-22)'s documented containment applied
to one directory and never to the other, and the other is where the fund-safety invariants live.
Filed as [H-42](DEFECTS.md#h-42); the fix is one line per target.

**Resolved, and the resolution is the diagnosis.** The hung run was killed and every leg of step 4
was re-run directly: `invariants` **45 passed / 0 failed in 63.68 s** at `--test-threads=1`,
`regressions` 6/0, `deposit_replay` 9/0 (1 ignored), `ui_limits` 6/0, plus `cargo test --workspace`
114/0 and the differential fast subset 22/0. So **the fast gate's content is green and its wrapper is
what failed**: the same target that finishes in one minute single-threaded blocked for thirty-three
at `--test-threads=2` with four other agents' PocketIC servers on the machine. A suite that spawns a
PocketIC per test, times out never, and is run two-wide is one busy machine away from an unbounded
hang, and the fix is the bound `cmd_test` already applies one step later.

### `make hygiene` is a coin flip, and both wave-5 reports were right

Two build reports say "repo hygiene clean" and two critics found `! repo hygiene FAILED`. Both were
right, minutes apart, and this pass reproduced both **on the same tree half an hour apart**:

```
before the definitive shots run:   42 untracked path(s),  1667 KiB total   ✓ repo hygiene clean
after  the definitive shots run:   44 untracked path(s), 13470 KiB total   ! repo hygiene FAILED
```

Two independent causes, either of which is now enough on its own:

1. **The rule counts modified tracked files as untracked payload.** `git status --porcelain` lists
   ` M path` as well as `?? path`, and this wave's modified sources are **7,261 KiB** of the 13,470.
2. **The occlusion gate's manifest is 5,972 KiB by itself**: `artifacts/screens/fe72d46/manifest.json`,
   which records every pixel-tested pair. Genuinely untracked payload is 6,209 KiB, so it fails even
   with cause 1 fixed.

**Every substantive check is green in both conditions**: no binary/media files, all four README
notices, all four frontend notices, the no-rake property, and "no notice line removed or altered
since `ceacc37`". The size heuristic is the only thing red, and it is red for reasons that have
nothing to do with large or binary files being committed. Fixes: [H-39](DEFECTS.md#h-39) (two lines
in `scripts/dev.sh`) and [L-06](DEFECTS.md#l-06) (write the per-pair detail beside the manifest,
not into it).

**Stated for the record so nobody has to guess: `./scripts/dev.sh hygiene` on the tree as this pass
leaves it returns `! repo hygiene FAILED` on the size rule, and every notice check inside it
passes.**

### The seam count, and one thing to watch

6 of 7 stands: the engine is byte-identical to `fe72d46`, so no mutation that died can have started
surviving. **This pass did not re-run the seven mutants** (each needs a mutant built against the
`801aa79` baseline) and says so rather than implying it did.

One harness change this wave is **not** additive and is worth the lead's attention.
`tests/money_safety/src/documented.rs`'s register went from **empty**: the state its own module
docs describe as the goal, *"nothing on the money path is excused"*: to one entry for E-36. The
entry keys on `(M1bPotBreakdown, canister_reports_its_own_inconsistency)`, and
`relational.rs:327-338` downgrades **every** `TOLERATED_SELF_REPORTS` match onto that one pair, so
the single entry is a blanket amnesty for the whole substring list, present and future. The pass's
own critic demonstrated a different engine warning being excused under E-36's id and E-36's reason.
The fix stays the one the register was designed for, one entry per tolerated line, enforced by
`register_entries_are_all_still_needed`: and that enforcement does not exist.

---

## 9. What is proven, and what is not

### Proven

- **The shuffle is verifiable by an outsider.** Now with the strongest possible evidence: a stranger
  wrote a verifier from the spec alone, ran nothing of ours, and reproduced three hands exactly,
  including predicting the turn and river from a commitment recorded at the flop.
- **No rake, and chips conserved to the e8** in every hand the auditor settled, independently of our
  own harnesses.
- **`withdraw` is reentrancy-safe**, admin queries reject non-controllers, mucked cards stay hidden,
  the dev faucet is dead, all confirmed by the auditor.
- **All five protected phrases are on screen and unoccluded on all 15 (surface, viewport) pairs a
  player can reach, and on all 22 shots of the definitive screenshot run at both
  viewports**, and this is now an assertion in the harness, proved to go red by mutation.
- **The playing surface is 60.6% (6-max) / 50.7% (9-max) of a 390×844 frame in the settled state**,
  with a floor now asserted centrally and proved to go red by mutation.
- **Nothing covers a money, equity or card figure**: 0 occluded across all 22 shots.
- Cross-version upgrades, hand ranking, and the settlement oracle's attribution, all unchanged from
  earlier waves and still green.

### Not proven, and flagged as such

- **That the deployed code is this code.** Nothing in this repository can establish it today
  ([T-33](DEFECTS.md#t-33)). This is the wave's most important negative result.
- **That a player can always get their money out.** No invariant in `tests/money_safety` asks the
  question, and the auditor reached a state where the answer was no
  ([FINDING 15](SECURITY-FINDINGS.md)).
- **That the Withdraw modal's notices and money copy reach a screen.** Untestable in the state a
  local player can reach.
- **That the BTC money surface works at all.** No local ckBTC ledger exists
  ([T-35](DEFECTS.md#t-35)).
- **That the hand replayer's ordering verdict means what it says.** Its own critic replaced the
  comparison with `const same = true` and the scene stayed green, still filing a canonical PNG.
- **That the pixel gate cannot be fooled.** Its own critic got 8 of 13 full-cover constructions past
  it, two recorded as non-gating at 83% and 100% suppression.
- **That any of this holds at 844×390 or 320×568.** The harness photographs two viewports.

---

## 10. Wave 6, ranked

1. **Make the deployed artifact identifiable, and fix the reproducible build.** Strip the `name`
   section or pin the build path; embed the git revision as canister metadata; add `src/poker_core`
   to the Dockerfile's `COPY` list and pin the base image; read `module_hash` over the public
   read-state path instead of a controller-only `canister status`. Until this is true, **nothing
   else in the repository can be trusted to describe the running system**, which makes every other
   item on this list conditional. ([T-33](DEFECTS.md#t-33))
2. **Close the fund lock, test first.** A `TableState` at `Showdown` with an empty board settled
   through `determine_winners`, red before and green after `try_evaluate_hand(...).ok()` at
   `lib.rs:871`; then the broader rule that no trapping evaluator is reachable from an update entry
   point; then the M9 REACHABILITY invariant, because "the player cannot reach their money" is
   currently not any invariant at all. ([FINDING 15](SECURITY-FINDINGS.md),
   [E-42](DEFECTS.md#e-42))
3. **Stop treating a missing heartbeat as an absence of obligation.** A disconnected seat that is
   still eligible for the pot is a client-controlled free run to showdown.
   ([E-43](DEFECTS.md#e-43))
4. **Fix the lobby's prices.** Six constants in `init_microstakes_tables` plus a redeploy. A player
   who picks 0.01/0.02 sits down at 0.05/0.10. It is also the only thing keeping both lobby scenes
   red. ([L-04 correction](DEFECTS.md#found-by-the-wave-5-coherence-pass))
5. **Make `verify_shuffle` unable to lie by accident**: name the parameters, return a `Result`, or
   accept either order and say which reading matched. Cheap, and it is the one call a
   non-technical player would make. ([E-44](DEFECTS.md#e-44))
6. **Wire the history canister** and stop pruning proofs at 100 hands. A fairness guarantee with a
   one-hour shelf life is not one. ([T-34](DEFECTS.md#t-34))
7. **Deploy a local ckBTC ledger.** It unblocks the entire BTC custody surface, which today nobody
  , including us, has ever exercised, and it is the only way T-26's corrected 11-sat minimum can
   be seen on a rendered page. ([T-35](DEFECTS.md#t-35))
8. **Draw the ring from a known `max_players`.** The lobby row the player clicked already carries
   it. ([T-32](DEFECTS.md#t-32))
9. **Make the gates trustworthy as gates**, three small changes with an outsized effect on whether
   anyone believes a green run: bound the money-safety targets the way the settlement ones already
   are ([H-42](DEFECTS.md#h-42), the primary gate hung for 33 minutes in this pass); stop a partial
   screenshot run from overwriting the full run's manifest and index
   ([H-43](DEFECTS.md#h-43)); and fix the two lines that make `make hygiene` a coin flip
   ([H-39](DEFECTS.md#h-39), [L-06](DEFECTS.md#l-06)).
10. **Give the register back its "one entry per tolerated line" property**, and make
    `register_entries_are_all_still_needed` enforce it.
11. **A landscape-phone scene and a 320-wide scene**, so the two viewports this wave fixed by hand
    have a gate.
12. **Reconcile funds held against liabilities recorded**, as an endpoint. Without it, a lost
    deposit and a test-instance surplus look identical. ([E-46](DEFECTS.md#e-46))

---

## 11. The single biggest reason a stranger would not trust this application with money

**You cannot tell what code is holding it.**

Not the fund lock, though the fund lock is worse in the moment. The lock is a bug with a one-line
fix and a testable shape. This is the thing that makes every other assurance in the project
unbankable: the deployed module hash matches no commit in the repository, the build is not
path-independent so two people building the same source get different answers, the published Docker
verification cannot compile because a crate is missing from its `COPY` list, and the procedure the
README hands the reader requires controller access the reader does not have.

The auditor demonstrated exactly why that is not a paperwork problem. It bricked a funded table on
the deployed binary, then replayed the identical state against the source in this repository and
**the source settled it correctly**. The program holding the money and the program offered for
inspection are different programs, and the deployed one is worse. Everything this project is proud
of is a statement about source code that nobody can connect to the canister their ICP is sitting in:
the provably-fair shuffle a stranger reproduced from the spec alone, the settlement oracle, no rake,
chips conserved to the e8, three waves of fund-safety work.

A verifiable shuffle inside an unverifiable binary is not a verifiable game. Fix identity first,
and the rest of this document starts to mean something to somebody who did not write it.
