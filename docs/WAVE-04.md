# Wave 4 — the coherence pass, and the truth for the lead

Six builders worked in parallel this wave. **Three of them edited
`src/cleardeck_frontend/src/lib/components/PokerTable.svelte`** — one on a BigInt throw, one on
mobile layout, one on all-in drama — and nobody drove the result. This document is what happened
when somebody did.

Everything below was produced by running code on this machine, against the real local canisters, on
2026-08-05. Every number states how it was obtained. Where a builder's claim did not survive
measurement it is marked **NOT REPRODUCED** and the measurement is given. Where a builder's claim
did survive, it says so and does not re-argue it.

- **Engine sources are byte-identical to `3253b67`.** `git diff 3253b67 -- src/table_canister
  src/poker_core src/lobby src/history` is empty. No engine behaviour changed this wave; everything
  that changed is the frontend, the harnesses and the documents.
- `table_canister.wasm` sha256
  **`5996594741afd63f88f38098c5461a24fbacafbda16278fc44bd3317a6c55c23`**, toolchain 1.90.0.

---

## 0. The headline, in four sentences

1. **A protected notice was off screen where it matters most, and it is fixed.** On a 390×844
   phone, on the table view and behind the open Deposit modal, **zero of the four
   player-protection notices were on screen**. Now four of four are, on both viewports, on every
   view. §2.
2. **Two of the wave's own surfaces made opposite claims about the same fact, and it is fixed.**
   `ShuffleProof.svelte` retired the commit-before-deal claim as *Not proven*; the hand replayer
   in `HandHistory.svelte` went on asserting it at full strength on the screen a player opens to
   review a hand they lost. §3.
3. **The mobile felt headline (42.9–52.1%) only exists in the configuration that hid the
   notices.** With the notices on screen — the only legal configuration — the wave-4 portrait
   redesign measures **18.3% (6-max) / 16.4% (9-max)** against **20.9%** for the geometry it
   replaced. On its own metric, as it must ship, it is a **regression**. §4.
4. **The engine is the strongest part of this project and can be left alone for a wave**, with two
   named exceptions. §8.

---

## 1. Reconciling `PokerTable.svelte` — what three editors did to one file

`git diff 3253b67` on that file is **+1209 / −110 lines**. The three passes are separable and, with
two exceptions, they composed cleanly. Read as three overlays:

| overlay | what it added | where it touches the other two |
|---|---|---|
| trust panel | imports `candidKey` from `$lib/utils.js` (which installs the app-wide `BigInt.prototype.toJSON` guard on import); rewrites `actionKey()` so no branch can throw | the import is the only line the other two could have deleted. It survived. |
| mobile surface | `PORTRAIT_AR` 0.70 → 0.555; `--ring-kx` 1.00 → 0.70/0.90/0.62; full-bleed wrapper; `anchorToTop()`; portrait transposition of the chip-disc and revealed-pair placement rules | the chip-disc transposition and the revealed-pair `cy` flip are edits to the SAME two expressions the all-in pass then read for `.winner-award` positioning |
| all-in drama | `$lib/equity.js`; `.equity-badge`, `.board-frame`/`.board-caption`, `.winner-award` (which absorbed the old `.hand-tag`), `.fold-word`, two flight animations, `isInHand()`/`liveSeats` | it re-parents `.hand-tag` into `.winner-award` and adds a portrait override for it; it reads `bx`/`by`, which mobile changed |

### 1.1 What I looked for, and did not find

Checked by reading the merged file and then by driving it:

* **A fix undone.** No. `actionKey()` has no `JSON.stringify` left; `candidKey` is imported and used;
  the guard is in the deployed bundle. Driven: **0 page errors and 0 console errors** across a
  complete hand at both viewports (`$SCRATCH/walk/report-*.json`).
* **Duplicated state.** No. One `portrait` signal, one `equityMemo`, one `reducedMotion`, one
  `lastStreetBets`. The only duplicated CSS selectors in the file are `.pot-flight` and `.main-pot`,
  and each pair is base rule + portrait override, which is the file's convention.
* **Competing responsive rules.** No collision found. Every portrait override the all-in pass needs
  (`.winner-award --award-out: 1.12`, `.stack-delta`, `.hand-tag`, `.caption-*`, `.equity-badge`,
  `.equity-method`) is present inside the `@media (max-aspect-ratio: 1/1)` block that mobile wrote,
  and the landscape values are untouched. Verified by measurement: desktop felt is
  **982.8 × 468, aspect 2.10, 35.5% of a 1440×900 window** after all three passes, matching the
  proven wave-3 geometry.
* **Effects that fight each other.** One was worth checking hard. `anchorToTop()` runs inside the
  same `$effect` as `measureViewport()`, and that effect reads `wrapperEl` — a `$state`. It does
  **not** read `tableState`, so it does not re-run on the 500 ms poll and does not fight the player's
  scroll. Measured: after arriving at a table, `window.scrollTo(0, 99999)` moves the page and it
  stays moved (`maxScroll` 330 desktop / 375–394 mobile). It is a mount-time correction, as
  documented.

### 1.2 The two places the parallel work genuinely collided

Both are in §5 with the pixels. Neither was caught by any gate in the repo, because both are
questions about **what covers what**, and every gate reads `textContent`.

---

## 2. The protected notices — the finding, the fix, and the counts

### 2.1 What I measured before touching anything

Live DOM, shipped build, real canisters, mobile 390×844 (layout viewport 425×918 because of
[T-19](DEFECTS.md#t-19)), on the **table view**:

```
.alpha-warning-banner   display:none                      0 px on screen
.footer-disclaimer      y 931..1166, height 235           0 px on screen, 248 px of scroll away
phrases on screen: Unaudited code with known bugs  NO
                   your funds are NOT safe          NO
                   18+ only                         NO
                   illegal in many jurisdictions    NO
phrases in the DOM: 4 of 4
```

**Identical with the Deposit modal open**, which is the screen a player commits real ICP from.

`make hygiene` was green throughout, and correctly so: it greps the **source** for the four phrases
and nothing was ever removed from the source. A notice that is present in the bundle and absent
from the screen is invisible to every check this repo has.

The wave-3 baseline does not have this problem: at `3253b67` the banner renders on the mobile table
view at y 0..268, fully in the viewport.

### 2.2 The fix (`src/index.scss`, a file this pass owns)

The de-duplication the mobile agent was permitted to do is **kept** — a phone still renders the four
notices once, not twice. What changed is *which* copy survives: it is now always the top banner,
which is above the fold on every view, instead of the footer, which is the last block of a long
document. One rule replaced two:

```scss
@media (max-aspect-ratio: 1/1) { .footer-disclaimer { display: none; } }
```

Desktop is untouched and still renders both.

**Verified after the fix, same probe, same build, same table:** banner `y 0..268`, fully in the
viewport, **4 of 4 phrases on screen** on the mobile table view and behind the open Deposit modal.

### 2.3 Protected-phrase counts

Lines containing each phrase, case-insensitive, over `README.md` + `src/cleardeck_frontend/src` —
the same scope `make hygiene` diffs. This reproduces the brief's baseline exactly.

| phrase | `3253b67` | wave 4 as built | **after this pass** | verdict |
|---|---|---|---|---|
| unaudited | 7 | 9 | **10** | up |
| 18+ | 5 | 7 | **8** | up |
| jurisdiction | 4 | 6 | **7** | up |
| own risk | 4 | 4 | **4** | unchanged |
| rake | 32 | 30 | **30** | **down 2 — justified below** |

**The two `rake` lines, named.** Both are in `Lobby.svelte` and both come from the lobby-density
pass, not from the mobile pass:

1. A three-line block comment about the no-rake claim was reflowed to two lines. Zero user-visible
   change.
2. The list bar's subheading went from `<span class="norake">no rake on any of them</span>` to
   `<span class="norake">0% rake</span>`.

Change 2 is the only one a player sees. **I judge it not a weakening**: it is the same property
stated as a figure rather than a phrase, in the same element, in the same position on the first
screen; and `0% rake` is the exact string the reference-beating headline claim uses. The no-rake
property is stated on the lobby in four other places, all intact: the `0%` hero claim, the
`0% rake` slim chip, `<strong>0% rake.</strong>` in the rake line, and
`<strong>No rake is taken from any pot on any table.</strong>` in the footer.

**What I do flag**: the `0% Rake. Every pot, every stake.` hero block **moved below the list** in the
lobby redesign. That is a prominence reduction for the headline no-rake claim, offset by `0% rake`
appearing in the list bar at the top. Net: acceptable, but it is the second time this wave that a
protected property lost vertical position to a layout goal, and the lead should know the pattern.

`make hygiene` is green. Nine protected notices present, no notice line removed or altered since
`ceacc37`.

---

## 3. Two trust surfaces, opposite claims — fixed

`ShuffleProof.svelte` (wave 4) now says, correctly:

> **Not proven.** *That the commitment came before the cards.* This page reads … off the table
> canister; it did not watch the order of events.

`HandHistory.svelte`, the per-hand replayer — the surface a player opens to review a hand they lost
— led with, at full strength and with a green tick:

> **These cards were fixed before the hand was played.** All 7 cards below were re-derived in this
> browser from the seed whose SHA-256 the table published at 05:45:17 AM — **before any card was
> dealt.**

Captured live at both viewports. The modal contained the words "not proven" **zero** times. In the
same modal, the first action-log line is timestamped **05:45:22 AM**, five seconds after the moment
the copy calls "before any card was dealt" — so the page has the evidence to know it is reading a
clock, and says otherwise anyway.

**Fixed.** The banner now states what the browser actually did, and carries the ordering as a
tagged limit inside the same green box, so the verdict cannot be read without the caveat:

> **These cards follow from the seed the table committed to.** All 7 cards below were re-derived in
> this browser from the revealed seed, and its SHA-256 recomputed here matches the commitment the
> table published for this hand. No canister was asked to confirm it.
> **[NOT PROVEN]** That the commitment came *before* the deal. The 05:45:17 AM above is a clock read
> off the table canister; this page did not watch the order of events … To witness it yourself, copy
> the commitment off the table while a hand is still running and compare it here after the seed is
> revealed.

Verified live at both viewports after the change: `notProvenInModal: true`.

---

## 4. The mobile felt: the headline number, and what it costs

This is the most important measurement in the wave.

**Method.** One tool, three variants, same build, same replica, same two tables, `getBoundingClientRect`
on `.felt` over `window.innerWidth × window.innerHeight`, mobile viewport 390×844. Variant C
reproduces the pre-wave-4 portrait geometry by injecting `3253b67`'s CSS constants (`--ar: 0.70`,
`--ring-kx: 1.00`, `--ring-ky: 0.98`, the three `--fw` caps, elliptical `.felt`, no full-bleed) —
legitimate because the felt box is a pure function of `--ar` and `--fw`, both of which are CSS;
`PORTRAIT_AR` in the script block only places the seat ring. Script:
`$SCRATCH/felt-ab.mjs`.

| variant | notices on screen | 6-max felt | 6-max % | 9-max felt | 9-max % |
|---|---|---|---|---|---|
| **A — shipped now** (wave-4 geometry, notices restored) | **4 of 4** | 199.1 × 358.7 | **18.3%** | 188.2 × 339.2 | **16.4%** |
| B — wave 4 as built (notices hidden) | 0 of 4 | 335.4 × 604.3 | 51.9% | 304.2 × 548.1 | 42.7% |
| C — `3253b67` geometry (notices on screen) | 4 of 4 | 238.9 × 341.3 | **20.9%** | 238.9 × 341.3 | **20.9%** |
| PokerNow `mobile-portrait-1`, real capture, same device | n/a | 313 × 548 | **52.1%** | — | — |

**Read that table twice.** Variant B is the one every headline in the wave-4 reports is computed
from, and it is not a shippable configuration. Between the two configurations that ARE shippable,
the redesign **loses**: 20.9% → 18.3% at 6-max and 20.9% → 16.4% at 9-max.

This reproduces the mobile pass's own critic to the tenth of a percent (they measured 18.3% / 16.4%
by DOM on an isolating variant they built independently), and it is the second independent
reproduction.

**Why it loses, precisely, and what to do about it.** A 0.555 aspect is the right shape *when there
is height to spend*. There is not: 268 px of banner + a 173.5 px three-row table header on a 918 px
layout viewport leaves the **height** cap binding, and at a fixed height a narrower felt has less
area. At 6-max, variant A's felt is 18 px **taller** than variant C's and 40 px **narrower**, and
loses on area for exactly that reason.

So the aspect is not the bug and reverting it is not the fix. Two things are:

1. **Recover the vertical budget from chrome that protects nobody.** [T-19](DEFECTS.md#t-19):
   `.header-right` needs 411.8 CSS px, the viewport meta has no `initial-scale`, so every phone
   renders the whole app at **0.918 scale** and ~32 device px of the right edge is blank. Fixing
   that alone returns 8.2% of linear scale, i.e. ~17% of area, and costs the player nothing. The
   three-row table header is worth ~120 CSS px more.
2. **Make `PORTRAIT_AR` a function of the stage height** rather than a constant, so the felt uses
   whichever aspect maximises area for the height it actually has.

Until one of those lands, the honest number for the mobile table is **16.4–18.3%**, and PokerNow
wins this scene by roughly **2.9 to 1**.

---

## 5. What the walk found that no gate can see

A complete new-user walk at **both** viewports, against the real local canisters: land on the lobby
signed out → sign in → enter a table → open Deposit → take a seat → play a hand to a showdown by
clicking the app's own buttons → open the hand history → open the per-hand replayer → verify the
shuffle → open Withdraw. Both walks completed. Script and raw output:
`$SCRATCH/walk.mjs`, `$SCRATCH/walk/report-{desktop,mobile}.json`, 11 PNGs each.

**Zero page errors, zero console errors, zero failed canister requests** on both walks. The trust
pass's guard holds through a full hand.

### 5.1 The equity badge is unreadable on a phone, and reads as a *different number*

In the gate's own accepted artifact `artifacts/screens/latest/table-showdown-mobile.png`, the
hero's badge says `0.00%` and **a player sees `0%`**, because the hero's own `8♠` covers the rest of
it. At the all-in the same badge is 66.5% covered. I confirmed the mechanism in the live DOM rather
than by eye: `document.elementFromPoint` at the centre of each badge returns
`div.player-cards` and `span.pip` — the cards — on **both** badges, at both the 6-max and 9-max
tables.

The cause is a stacking collision between two of this wave's three editors. Inside a `.seat`,
`.player-nameplate` is `z-index: 6` and therefore a stacking context; the mobile pass placed
`.equity-badge` **inside** it at `z-index: 3`; the all-in pass draws revealed and hero cards at
`z-index: 7`. The badge can never win from where it is.

**Not fixed here.** Every candidate fix moves either the plate or the badge's containing block, and
this pass is not authorised to redesign the portrait pod. It is wave 5's first mobile item, with the
exact repro above. A truncated percentage that reads as a plausible different percentage is worse
than showing nothing.

### 5.2 The winner's award covers the cards that justify it

Same artifact: the `+24.00` chip sits on top of the winner's revealed pair — the red `8` of a
revealed card is visible peeking out behind the chip. On desktop the same element lands on the
**board**: measured live, `.winner-award` overlaps `.community-cards` by **25.6% of the award's own
area** when the winner is at seat 1 or 2, and by 0% when the hero at seat 0 wins — which is why one
run of the walk saw it and one did not.

### 5.3 The equity method line vanishes exactly when the equity becomes a verdict

`.equity-method` (`EQUITY · EXACT · 1 RUNOUT`, or the `vs N random · MONTE CARLO` statement) lives
inside `.pot-display`, which is replaced by the winner banner once the pot is zero. Measured at the
showdown on both viewports: `.equity-method` is **null** while two solid `100.00% / 0.00%` badges
are on screen. The method survives only in a `title` attribute, which a phone has no way to open.
The honesty framing the feature was built on is missing from the one frame that is photographed.

### 5.4 The mobile action log named nobody — fixed

`HandHistory.svelte` hid `.log-seat` below 560 px, so the phone rendered every line as
`06:03:57 AM calls`. Eight of eight lines anonymous; the log could not answer the only question it
exists to answer. **Fixed**: the time column gives up 10 px instead, and the actor is back on 8 of 8
lines on a phone.

### 5.5 The shuffle verdict is below the fold on both viewports

`.headline` (`Verified on your machine. This page asked no one's permission to say so.`) reaches
`4 of 4` rungs and a green verdict on both viewports — that part of the trust pass is real and I
reproduced it. But `headlineInViewport: false` on **desktop as well as mobile**. On mobile the cause
is structural, not scroll position: the panel sits in `.proof-sidebar` with `clientHeight 918` and
`scrollHeight 2708` while `document.scrollHeight` is only 1293, so the verdict at `y=1717` cannot be
reached by scrolling the page at all — only by scrolling a nested container a player has no reason
to know is there.

### 5.6 The action log is still not a replayer's log

From a hand I drove myself: 8 log lines, 8 of 8 carry a timestamp and (now) an actor, **0 carry an
amount**, **0 blind posts**, **0 street markers**. The component says so itself
(`Blind posts are not recorded as actions at all`). Against PokerNow's session log, which puts the
amount on 4 of 4 money actions, names a player on 13 of 13 lines and prints `Flop: [3♥, 10♥, 3♦]`
inline. This is the scene the wave lost, and the reason it was never seen is that the `handhistory`
screenshot scene **never clicks a hand open**.

### 5.7 Withdraw states a minimum it does not enforce

`WithdrawModal.svelte:15` enforces `minWithdrawal = isBTC ? 11n : 100000n`. The copy on line 179 and
the input's `min` attribute both say **1,000 sats**, and the error string says
`Minimum withdrawal is 1,000 sats`. The stated minimum is **90.9× the enforced one**, and the error
message is unreachable for every amount between 12 and 999 sats. ICP is consistent (0.001 ICP =
100,000 e8s). BTC tables are local-only today, which is why this is `medium` and not higher.

---

## 6. Gates — every one the brief named, run on this tree

| gate | command | result |
|---|---|---|
| fast gate | `./scripts/dev.sh test` | **green, exit 0.** workspace (18 targets), wasm build, differential fast (25 + 22, 2 ignored), money-safety invariants 45 / regressions 6 / deposit_replay 9 (1 ignored) / fuzz smoke 1, settlement 9, disagreements 8 |
| settlement oracle | `make settlement` | **green.** 3 passed, 5 filtered; per-seat, per-principal and per-record columns all `+0` |
| known defects | `make known-defects` | **1 of 1 still red on purpose** (E-13 `detect_straight`). Unchanged |
| hygiene | `make hygiene` | **green.** 9 protected notices present; no notice line removed or altered since `ceacc37` |
| mainnet guard | `make selftest` | **green.** 7 mainnet ids refused in every hostile shape; 3 legitimate shapes allowed |
| money suite + sha256 | `cd tests/money_safety && cargo test --test invariants --test regressions --test deposit_replay` at `CLEARDECK_TABLE_WASM=…` sha256 `59965947…c55c23` | **green.** 45 / 6 / 9 (+1 ignored) |
| money fuzzer, **its own default** | `cd tests/money_safety && cargo test --test fuzz` | **RED.** Reproduces [H-28](DEFECTS.md#h-28) exactly: seed `0xc1ea2dec0003`, shrunk 220 → 12 ops, `M1b_POT_BREAKDOWN:canister_reports_its_own_inconsistency`. Not in any make target, so no green gate hides it — but the suite's own front door is red |
| **seam mutations** | `$SCRATCH/seam_mutations.py` on a `cp -Rc` copy | **6 of 7 die. Not down.** Same survivor (mutation 5, the dropped self-report). `lib.rs` asserted byte-identical afterwards |
| screenshot harness | `./scripts/dev.sh shots` | 20 shots, **18 verified, 2 UNVERIFIED**. `0 UNASSERTED` tokens in all 20. The two reds are `lobby` desktop + mobile, correctly red for the stale table-name defect |

**Seam mutation detail** (each mutation applied alone, restored between, `lib.rs` restored
byte-identical, gate = `cargo test --workspace` then `cd tests/money_safety && cargo test --test
invariants`):

| mutation | verdict | convicted by |
|---|---|---|
| 1. the payout basis is never rebuilt | **died** | `seam_a`, `seam_b` (42 passed, 3 failed) |
| 2. the payout basis is halved | **died** | `cargo test --workspace` (20 of 23 failed) |
| 3. the breakdown written into a throwaway `Vec` | **died** | `seam_a`, `seam_b` |
| 4. an empty contribution slice builds the pots | **died** | `cargo test --workspace` (14 of 23 failed) |
| 5. the self-report is dropped | **SURVIVED** | — (equivalent mutant, argued in [H-04](DEFECTS.md#h-04)) |
| 6. `evaluate_hand` stubbed to `RoyalFlush` | **died** | `cargo test --workspace` (8 of 23 failed) |
| 7. every hand from `b"CONSTANT"` | **died** | `seam_c`, `seam_honest_play…` |

This is the third consecutive wave at 6 of 7 with the same survivor, and the engine did not change
this wave, so the result is a control rather than news.

---

## 7. Scene-by-scene A/B against the four reference clients

`✓` = I measured it myself this pass. `↩` = the builder's or their critic's measurement, reproduced
in spirit but not re-run here. Percentages of the playing surface are area over the frame.

| scene | competitor | who wins | the number |
|---|---|---|---|
| **desktop table**, 1440×900 | PokerNow / GGPoker reference median 32.0% | **ours** ✓ | felt 982.8 × 468, aspect **2.10**, **35.5%** of the window, nothing off-frame. Unchanged by all three PokerTable passes |
| **mobile table**, 390×844, notices on screen | PokerNow `mobile-portrait-1` 313×548 = 52.1% | **theirs, 2.9:1** ✓ | ours 18.3% (6-max) / **16.4%** (9-max). Also a regression on `3253b67`'s 20.9% |
| **mobile table**, notices hidden (not shippable) | same | ours ✓ | 51.9% / 42.7%. Recorded because it is the number every wave-4 report quotes |
| **mobile stack legibility** | PokerNow 8 px digits | ours ↩ | 11–12 px measured ink vs 8 px. Two 9-max pods at 11 px are still under BAR 24's own 12 px floor, and the 14 px claim was computed font-size, not glyph ink |
| **all-in, desktop** | GGPoker `web-gg-wpd2-2` | theirs ↩ | GG prints the **exact** 100.00% because it turns all-in hands face up; ours prints a dashed modelled `vs 2 random` figure that the critic measured 15.4–19.9 points optimistic on the captured spot. We win on committed-chip discs and a pot-odds row GG has nothing like |
| **all-in, mobile** | PokerNow `allin-live-1` | **theirs** ✓ | our equity badge is covered by the hero's own cards (`elementFromPoint` returns `.player-cards`). An unreadable decision number loses to an honest absence |
| **showdown, desktop** | PokerNow `showdown-winnerglow-live-1` | ours ↩ | both exact equities, hand named in words, winner banner, award chip, 4 s glow (theirs 5.6–7.0 s). Their `+450` sits inside the winner's plate; ours lands on the board at **25.6% of its own area** for seats 1–2 ✓ |
| **showdown, mobile** | PokerNow | **theirs** ✓ | the award chip covers the winner's revealed pair; the hero's `0.00%` renders as `0%`. Seven-card row genuinely gone (board and revealed pair 114 px apart) |
| **hand-history action log** | PokerNow Session Log | **theirs** ✓ | ours 8 lines, 8 with a timestamp, 8 with an actor (fixed this pass), **0 with an amount, 0 blind posts, 0 street markers**. Theirs: amount on 4 of 4 money actions, player named on 13 of 13, inline `Flop:` marker |
| **per-hand verification inside the replayer** | PokerNow footer controls | **ours** ↩ | deck position + tick beside every card the hand exposed, three separately copyable hashes. No competitor in the corpus can copy this without building the same machine |
| **hand list as an index** | GGPoker PokerCraft | theirs ↩ | they expose 7 columns + Hand ID search + player search over the whole history; ours exposes 7 fields, 2 filter chips, **0 search inputs** ✓, and `HandHistory.svelte` `const WINDOW = 20` |
| **shuffle verification surface** | nothing equivalent exists in the corpus | **ours**, with a caveat ✓ | 4 of 4 rungs and a green verdict at both viewports — but `headlineInViewport: false` on **both**, and on mobile it is inside a nested scroller the page cannot reach |
| **lobby, desktop** | PokerStars `web-ps-wpd2-1` (34.4%, 24 rows, 9 columns) | theirs ↩ | ours first row at y=370 = **41.1%** ✓, 3 of 3 rows fully visible ✓, 7 fields, 18-fact preview, 2 stale-name strikethroughs ✓ |
| **lobby, desktop** | WPT Global (26.4%, 5 of 5 rows, 4 fields) | tie ↩ | they win position, we win density |
| **lobby, mobile** | **PokerStars `web-ps-gipsy-2.png`** — a real mobile client capture | **theirs, decisively** ✓ | theirs: first row y≈116 of a 568 px screen = **20.4%**, 4 rows fully visible + a 5th clipped, pitch ~85 px. Ours: first row y=543 = **64.3%**, **1** row fully visible. See §9 — the doc says this capture does not exist |

---

## 8. Is the engine safe to leave alone?

**Yes, for one wave, with two named exceptions.** The reasoning, not the reassurance:

* The engine did not change this wave and every money gate is green on it: settlement oracle
  per-seat **and** per-principal **and** per-record all `+0`; 45 invariants; 6 regressions; the
  E-02 fund-theft reproducer and its ten regressions; 6 of 7 seam mutations, third wave running.
* Attribution stopped being a property of two fixtures and became a property of every settled hand.
  Six conserving misdirection bugs were planted one at a time; before, the money-safety harness
  caught **1 of 6** and `push_winner`'s owner-merge escaped every gate in the repo; after, **no
  plant escapes**. That is the single largest real improvement in the wave.
* Hand ranking, shuffle verifiability, upgrade survival and payout basis are all settled ground
  from earlier waves and nothing this wave disturbed them.

**The two exceptions, both of which are money and neither of which is closed:**

1. **[E-41](DEFECTS.md#e-41)** — a hostile sequence ends with the canister **short by 2,000,000 e8s**
   against its own ledger. M2 LEDGER REALITY is never excusable. It reproduces on a pristine
   `git archive HEAD` tree. Wave 4 did not root-cause it, but wave 4 *did* remove the excuse: the
   attribution critic showed a fuzz run **is** a pure function of `(seed, config, actors, steps)`
   provided the seed keeps its list position, so [H-26](DEFECTS.md#h-26) does not block shrinking
   E-41 and the one-line fix is to key `config_for()` off the seed instead of its index.
2. **[H-28](DEFECTS.md#h-28)** — the money fuzzer's own default invocation is **red**, and I
   reproduced it. A suite whose front door is red is a suite people stop opening.

Everything else on the engine can wait. **The frontend cannot**: the two things this pass fixed were
both cases of a player being shown something untrue or being shown nothing where a warning belongs,
and neither was visible to any gate.

---

## 9. Corrections to `docs/DESIGN-BAR.md`

* **9.4.1's "There is no real mobile lobby capture in the corpus" is false.**
  `$SCRATCH/reference/pokerstars/web-ps-gipsy-2.png` is indexed `client=pokerstars`,
  `real_gameplay=true`, `capture_type: third-party review (real client screenshots)`, notes
  "Real PokerStars mobile client: lobby list, Spin&Go table, store". Its left panel is a complete
  phone screen. I opened it and measured it: screen rows 0..567, first tournament card at **y≈116 =
  20.4%**, row pitch ~85 px, **4 rows fully visible** and a 5th clipped by the bottom tab bar. The
  doc missed it by querying `INDEX.json` for `scene == 'lobby-mobile'`; this file is filed
  `scene == 'mobile-portrait'`. **Consequence:** §9.4 set no mobile first-row bar, on a false
  premise, for exactly the metric ClearDeck fails worst. BAR 31 now exists.
* **BAR 22's "today 19.7–21.1%" and the mobile pass's "now 42.9–52.1%" are both superseded** by §4:
  in the only shippable configuration the number is **16.4–18.3%**.
* **BAR 25's derivation over-charges the protected disclaimer by 2.9×.** It attributes 160 px
  (17.8% of 900) to protected copy; the paragraph carrying the four hygiene-enforced notices is
  55.5 px on desktop and 109.5 px on a phone. The rest is `.banner-info` and `.banner-ai`, which
  no rule protects. Same conflation produced the mobile "46.0% arithmetic floor".
* **BAR 24's reference figure ("PokerNow portrait's 15 px") is not reproducible.** Independent
  detectors read 8 px on every PokerNow stack and 12 px on its largest money glyph anywhere.

---

## 10. Wave 5, ranked

Ordered by *harm to a player per hour of work*, not by how interesting the work is.

| # | item | why it is here | size |
|---|---|---|---|
| 1 | **[T-22](DEFECTS.md#t-22) the mobile equity badge reads `0%` when it means `100.00%`** | the only defect this wave that shows a player a **plausible wrong number** on a decision surface. Fix the portrait z-order/anchoring, then add one pixel assertion to the gate: fail the scene if any element with a higher effective z-index intersects `.equity-badge`'s client rect | S + S |
| 2 | **[T-19](DEFECTS.md#t-19) reclaim the phone's vertical budget** | this is the whole mobile scene. `initial-scale` + a `.header-right` that fits + a two-row table header returns ~120 CSS px and 8.2% of linear scale, and buys back most of the 4.5 points §4 gave up. **Costs the player nothing** | M |
| 3 | **[E-41](DEFECTS.md#e-41) shrink and root-cause the 2,000,000 e8s shortfall** | the only open finding where the canister owes more than it holds. Now unblocked: key `config_for()` off the seed (one line) and shrink | M |
| 4 | **[H-28](DEFECTS.md#h-28) make the fuzzer's own default green** | connect `TOLERATED_SELF_REPORTS` to `REGISTER`. Until then the money suite's front door is red and people learn to skip it | S |
| 5 | **[T-23](DEFECTS.md#t-23) the award chip covers the cards it is about** (mobile) and the board (desktop, 25.6%) | the second unreadable-money defect; same root cause as #1, same fix family | S |
| 6 | **the `handhistory` scene must open a hand** | the replayer is the surface where the action-log A/B is lost and where §3's over-claim hid for a whole wave, and no scene has ever photographed it | S |
| 7 | **[T-24 →] the action log: amounts, blind posts, street markers** | 0 of 8 lines carry an amount against PokerNow's 4 of 4. Blind posts need an engine change (`start_new_hand` does not record them as actions) | M |
| 8 | **[T-26](DEFECTS.md#t-26) WithdrawModal's BTC minimum, 90.9× wrong** | one line, and it is a money figure the app states and does not enforce | XS |
| 9 | **the token census must read attributes and elements, not text nodes** | `1037 tokens, 0 UNASSERTED` means 1037 of the tokens the walker visits. Money in `title`/`aria-label` is invisible (proved: a 9.99/19.98 tooltip left the census byte-identical), and `{whole}.{frac}` splits a money figure into two integers the allowlist excuses | M |
| 10 | **the shuffle verdict above the fold** | 4 of 4 rungs green and the headline off screen on **both** viewports; on mobile inside a nested scroller the page cannot reach | S |
| 11 | **[T-27](DEFECTS.md#t-27) the equity method line at showdown** | the method vanishes exactly when the badge becomes a verdict. Move it out of `.pot-display` | XS |
| 12 | **CI ([H-23](DEFECTS.md#h-23))** | every fund-safety result in these documents comes from harnesses no CI job invokes. It has been true for three waves | M |

**What I would tell the lead in one line.** The engine is in good shape and the harnesses got
materially better this wave; the client is where the remaining risk is, and both of the client
defects this pass fixed were invisible to every gate in the repo because every gate reads text and
neither defect was about text.
