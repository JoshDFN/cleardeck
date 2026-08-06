# Wave 3 — the coherence pass: one product, and the truth about it

Five agents worked wave 3 in parallel, blind to each other. Four restyled a different surface of
the same SvelteKit client; the fifth rewrote the payout path underneath them. This document is
written by a reader who built none of it. Everything below was **re-run, re-measured or
re-observed** here, against one binary whose sha256 is stated and one local replica. Where a
builder's summary disagrees with this file, this file says why.

Read with [DEFECTS.md](DEFECTS.md) (the register), [SECURITY-FINDINGS.md](SECURITY-FINDINGS.md)
(the evidence) and [DESIGN-BAR.md](DESIGN-BAR.md) (the measured bar).

> **Still unaudited alpha software.** Nothing in this wave changes that. Nobody has ever played on
> mainnet, and nothing here says anyone should.

Canister under test: `table_canister.wasm`
**sha256 `5996594741afd63f88f38098c5461a24fbacafbda16278fc44bd3317a6c55c23`**, built
2026-08-04 17:29:47, the same binary every gate below ran against.

---

## The one-paragraph answer

**The engine is in the best state it has ever been in, and the client is not one product yet.**
Wave 3's engine work is real: both upgrade blockers are closed as one change, and the "wrong player
gets paid" question that wave 2 opened is now closed by construction — `principal_of(state, seat)`
is *deleted*, so no function in `lib.rs` can turn a seat index into a payee. Six of seven seam
mutations still die, and I reproduced that independently. **But the client fails as a walk.** Doing
the whole journey once — land, sign in, deposit, sit, play a hand, open the history, verify the
shuffle — surfaced things no single builder could see: **19 uncaught BigInt exceptions in one
ordinary hand**, which killed the action log and starved the two features another agent had just
built; the **largest string on every table screen quoting blinds that were 5× and 10× wrong**, with
eighteen scenes filed "agrees with chain: yes" around it; a **canister trap observed live** on the
one `evaluate_hand` call site wave 3 forgot to guard; and, most quietly, the fact that **the app a
human opens in a browser does not work at all** — every screenshot this project has ever taken went
through a shim no user has. Four of those are fixed here and gated. On look and feel the desktop
table is now genuinely competitive and the fairness panel is better than anything in the reference
corpus; **the phone is where this loses, and it loses by 2.5×.**

---

## 1. Gates: every number, re-run here

| gate | command | result |
|---|---|---|
| fast gate | `./scripts/dev.sh test` | **green**, exit 0. `all fast gates green` |
| settlement oracle | `make settlement` | **green**, exit 0. 21 + 5 + 9 + 3 tests, 0 failed |
| known defects | `make known-defects` | exit 0. **1 of 1** marker still red — see H-25, this is nearly vacuous |
| hygiene | `make hygiene` | **green**. 9 notice checks, `no notice line removed or altered` |
| mainnet guard | `make selftest` | **green**. 20 hostile argument shapes refused, 3 allowed |
| seam mutations | `$SCRATCH/seam_mutations.py` on a `cp -R` copy | **6 of 7 die**, same survivor as wave 2 |
| screenshots | `./scripts/dev.sh shots` | exit 1. **9 of 10 scenes verified**, 244 money figures; `lobby` correctly UNVERIFIED |

Inside the fast gate, the suites that matter, all against the sha above:

```
money_safety  tests/invariants   39 passed; 0 failed        (incl. m8_convicts_a_misattributed_payout,
                                                              m8_a_departed_stake_is_paid_to_its_owner…,
                                                              m8b_an_ordinary_hand_pays_nobody_who_did_not_stake,
                                                              pre_upgrade_refuses_rather_than_proceeding…)
money_safety  tests/regressions   6 passed; 0 failed
money_safety  tests/deposit_replay 9 passed; 0 failed; 1 ignored
table_canister tests/betting_rules 25 passed
table_canister tests/coherence_regressions 22 passed; 2 ignored
differential  fast_subset          9 passed
```

**Seam mutations, re-run independently.** The wave-2 critic's applier (`c2mutate.py`) no longer
compiles against this tree — the FINDING 13 rewrite moved the functions it targeted — so I used the
wave-3 translation, on a copied tree, one mutation at a time, restoring between each and asserting
`lib.rs` byte-identical afterwards (it was):

| mutation | verdict | convicted by |
|---|---|---|
| 1. the payout basis is never rebuilt | **died** | `seam_a`, `seam_b` |
| 2. the payout basis is halved | **died** | `cargo test --workspace` (20 of 23 failed) |
| 3. the breakdown written into a throwaway `Vec` | **died** | `seam_a`, `seam_b` |
| 4. an empty contribution slice builds the pots | **died** | `cargo test --workspace` (14 of 23 failed) |
| 5. the self-report line dropped | **SURVIVED** | — (equivalent mutant, argued in H-04) |
| 6. `evaluate_hand` stubbed to `RoyalFlush` | **died** | `cargo test --workspace` (8 of 23 failed) |
| 7. every hand from the seed `b"CONSTANT"` | **died** | `seam_c`, `seam_honest_play…` |

**6 of 7. Unchanged from wave 2 and independently reproduced, which is the point** — the payout
path was rewritten wholesale and the mutation score did not regress.

---

## 2. The hard-rule check, done properly

The brief's rule 2 is the one a redesign quietly breaks. It did not break.

**Diffed against `ceacc37`, by hand, not by `make hygiene`:**

```
git diff ceacc37 -- README.md src/cleardeck_frontend/src | grep '^-' \
  | grep -Ei 'unaudited|18\+|jurisdiction|rake|NOT safe|own risk|illegal'
```

returns **two lines**, and neither is a notice: `<strong>Immutable History</strong>` and
`<strong>Non-Custodial</strong>` — two **false claims** that the lobby agent removed and replaced
with a "What This Does NOT Guarantee" section. Removing a false safety claim is a strengthening,
not a weakening, and it is the right call: the table canister holds a per-principal balance, so
"Non-Custodial — we can't access them" was untrue, and the canisters are controller-upgradeable, so
"Immutable History … cannot be altered" was untrue.

**Prominence, judged rather than grepped.** `git diff ceacc37 -- src/.../+page.svelte` for anything
matching `banner|disclaimer` returns **zero lines**. The disclaimer block, its CSS, its position and
its two copies are byte-identical to the baseline:

* `+page.svelte:666` — the top banner. Measured live: **183 px** at 1440×900, above the header, red
  ground, `⚠️ DISCLAIMER:` in bold, first thing on the page.
* `+page.svelte:835` — the identical block in the footer.
* `HowItWorks.svelte:246–251` — the whole notice repeated inside the modal, which is **new this
  wave** and an addition.
* README line 57 — unchanged.

The no-rake property was made **materially more prominent**, which the brief explicitly allows: it
is now the hero's display figure (`0%`, 34 px), a chip in the signed-in bar, a line in the lobby
preview instantiated with real money, a felt watermark, and the opening banner of the How It Works
modal.

**Verdict: nothing was weakened. Four notices intact in three places, one of them new.**

One flag for the lead, not a violation. The disclaimer block renders **twice on every page**, 183 px
each on desktop and 235 px on the phone, where it is 27.8% of the first screen. It is the single
largest recoverable block of mobile real estate (T-16). Deleting the second, redundant copy would
still pass `make hygiene` — but whether that weakens prominence is a judgement that belongs to the
lead, so this pass did not make it.

---

## 3. Walking the whole app as a new user

Nobody had done this. It is where most of what follows came from. Real Chromium, real local
canisters, one identity, in order.

### Seam 1 — the app does not open. **T-14, open, and it is the biggest one.**

`http://<frontend-id>.localhost:8077/` — the actual URL of the actual deployed asset canister —
renders an unstyled toast covering a third of the viewport with a raw stack trace
(`Failed to fetch HTTP request: TypeError: Failed to fetch at window.fetch
(…/_app/immutable/chunks/CjFkTVZV.js:1:1669) at requestFn …`), then `0 tables · 0 of 0 seats taken`
and an empty state that reads **"The lobby canister is reporting no tables."**

The lobby canister is reporting three. I asked it. The bundle points its agent at
`127.0.0.1:4943`; the gateway is on `8077`. `buildEnvFor` never sets `VITE_LOCAL_GATEWAY_PORT`, so
T-03's fix ("it now comes from the build environment") is inert.

**Every screenshot in `artifacts/` was taken through a reverse proxy the harness starts on 4943.**
The proxy's own header says the frontend source could not be edited because "a later wave redesigns
it". Wave 3 was that wave. So the gate has never once tested the thing a person can open, and the
one screen whose entire argument is "check this yourself" blames the chain for the client's own
misconfiguration.

Everything below was therefore done through the same shim the gate uses.

### Seam 2 — the lobby is one row deep

Measured with `getBoundingClientRect` at 1440×900: disclaimer 183 px, header 83 px, hero and
heading block to y=753, **first table row at y=796 — 88.4% down the viewport, one row visible.**
PokerNow's real lobby at 1512×945 puts its first row at y≈341, **36.1% down, with seven rows
visible** (measured from `reference/pokernow/lobby-community-1.png`). At 390×844 ours is worse:
first card at **y=1004, i.e. 119% of the viewport — zero rows before any scroll** — and the filter
strip clips (`scrollWidth 434` vs `clientWidth 366`, so "Micro" is sliced and "Low" is off-screen
with no fade or arrow).

### Seam 3 — depositing says nothing

Deposit modal, `12 ICP`, submit. The table balance goes `11.90 → 23.90 ICP`. **That is the entire
feedback.** No toast, no confirmation, no receipt, no modal success state. The one number that
changed is 1,200 px away in the bottom-right corner. The wallet balance in the same modal reads
`30.0004 ICP` and the minimum reads `0.0002 ICP (Network fee: 0.0001 ICP)` — four decimals — over a
table balance rendered at two. That is wave 2's "50.00 next to 0.0000", still live, just moved
(T-15).

### Seam 4 — sitting down has no buy-in step

`.join-seat` reads `SIT / SEAT 3`. Clicking it seats you immediately, for an amount you were never
shown or asked about. No buy-in dialog appeared (`buyinDialog: 0`). Every reference client asks.

### Seam 5 — 19 uncaught exceptions per hand. **T-10, fixed here.**

One heads-up hand at `table_2`, counted with `page.on('pageerror')`: **7 uncaught
`TypeError: Do not know how to serialize a BigInt` before any click, 12 more from one click on
`Call 0.10`.** `document.querySelectorAll('.feed-item')` returned 0 throughout; the action log sat
on its "Waiting for action…" empty state for the whole hand.

Cause: `JSON.stringify(lastAction.action)` inside an `$effect`, where `action` is a Candid variant
carrying `nat64` amounts. The throw is uncaught, the effect dies, nothing reaches `actionFeed`.

This is the mechanism behind a **competitive deficit another agent recorded as a design gap**. The
A/B row "5 of 5 action lines carry no amount; PokerNow interleaves the streets" is not a missing
feature. With the throw removed and nothing else changed, the same hand produces
`▲ Seat 2 raised to 0.20` / `→ Flop` / `☎ You called 0.10`. **Fixed and measured: 0 errors before
the click, 0 after.**

### Seam 6 — the canister trapped. **E-40, open, high.**

Four consecutive polls against the live `4zfnl-5t777-77775-aaadq-cai` returned

```
Reject code: 5  Canister called `ic0.trap` with message: 'Panicked at 'IMPOSSIBLE HAND:
evaluate_hand needs a 3-, 4- or 5-card board (flop/turn/river), got 0 community'
```

Wave 3's payout rewrite guarded two of the three `evaluate_hand` call sites in the canister
(`rank_claims` at `lib.rs:4090` and the winner record at `4503`). It missed the third, at
`lib.rs:871` in `record_hand_to_history`, where the only guard is `went_to_showdown && !has_folded`.
The frontend's poll loop opens with `check_timeouts()`, an **update**, so a trap there rolls the
call back and **the hand cannot settle**.

I did not isolate the minimal reproducing sequence, and I say so plainly: a scripted heads-up
timeout fold-out settles through `end_hand_single_winner` with `went_to_showdown = false` and does
not trap. What I have is a live observation on a real canister during ordinary browser play, and a
call site whose two siblings are guarded and which is not. Fix the guard, then find the sequence.

### Seam 7 — the withdrawal receipt lied about the amount. **T-18, fixed here.**

`withdraw` returns the **ledger block index**; the modal ran it through the ICP formatter. Measured
on the real local ledger: withdrawing 1 ICP returned block 1130, and 1130 e8s at four decimals is
**"Withdrawal successful! 0.0000 ICP sent to your wallet."** The wallet also receives `amount - fee`
and no screen said so. Now: `Withdrew 1.0000 ICP. 0.9999 ICP reached your wallet after the 0.0001
ICP network fee. Ledger block #1130.` — with an escrow delta of exactly `100_000_000` e8s to match.

Note what this says about the gate: `deposit` is a scene, **withdraw is not**, so the one screen in
the product that tells a player how much money left the table was never photographed.

### Seam 8 — three of four dialogs ignore Escape. **T-13, fixed here.**

Escape closed `HowItWorks` and did nothing in `DepositModal`, `WithdrawModal` and `HandHistory` —
each of which carried a keydown handler on a `tabindex="-1"` backdrop that nothing can focus. Found
as a Playwright timeout: the deposit backdrop stayed up after Escape and **swallowed the next click
on every header button.** All four now share one contract.

### Seam 9 — the fairness panel, and what the gate accepts

The verification is real, and better than anything in the reference corpus. On a quiet table it
reaches its verdict **untouched in 505 ms** (`.headline.good`, "Verified on your machine. This page
asked no one's permission to say so."), and I ran a controlled A/B — arm A untouched, arm B with a
BigInt-safe `JSON.stringify` shim installed before any app code — which gave **505 ms and 502 ms**,
i.e. the throw was not what stopped it in that state.

But **both shuffleproof PNGs shipped by wave 3 show the panel dead**: rungs 3 and 4 grey, subtitle
stuck on "Re-deriving your cards locally", no deck grid, no derived-vs-dealt pair — filed under the
canonical verified filename. The scene asserts `.proof-item >= 2`, which is satisfied by rungs 1 and
2 **before the browser computes anything**, and `settle()` waits for fonts and two frames. The
shutter fires first. After this pass's re-run the same scene captured the *live* state — rungs 3 and
4 green, both hashes identical, hole cards derived-vs-dealt with a `=`, five board cards each
stamped with its deck position and a tick, "7 of 7 cards" — **and the gate would have accepted
either.** That is H-24, and it is a real hole.

#### H-24 is closed, and both halves of it were measured

The scene now asserts the **verdict** instead of the scaffolding: `.headline.good`, four of four
rungs `.done`, the two hashes identical *and* equal to the `seed_hash` re-read over Candid in the
harness process, the hole cards compared as text across three independent sources (derived here vs
dealt by the canister vs painted on the felt), every board cell ticked, `N of N` in the tally, and
52 slots in the deck grid. The grid is expanded, counted and collapsed again with an in-page
`el.click()` so the published PNG is framed exactly as before.

Both states were then measured on the real canisters, with the *same* scene script:

| | pre-fix (guard off, `JSON.stringify(lastAction.action)` restored) | fixed |
|---|---|---|
| subtitle | "Re-deriving your cards locally" (frozen) | "Checked in your browser, not by us" |
| rungs done | **2 of 4** | 4 of 4 |
| derived-vs-dealt pairs | **0** | 2 |
| tally | **absent** | "7 of 7 cards" |
| `.headline` | **never appears** | `.headline.good` |
| uncaught page errors | **90** `Do not know how to serialize a BigInt` | 0 |
| **old assertion** (`.proof-item >= 2`, `.hash.revealed >= 1`) | **PASSES** | passes |
| **new assertion** | FAILS (45 s timeout waiting for a verdict) | passes |

The old gate certifies the dead panel; the new one cannot. Separately, `table-facing-bet` staged in
that same pre-fix state now fails with *"threw 7 uncaught error(s) while it was being staged"* —
`shoot()` refuses to photograph a page that threw (`tools/shots/lib/page-health.mjs`), so the class
is caught in **every** scene, not just this one.

### Seam 10 — "nobody chose the deck after seeing hole cards" was not proven

The fairness panel listed that under **Proven**. It is not, and the difference matters more here
than anywhere else in the product, because this panel's whole value is that it does not ask to be
believed.

**What the browser actually establishes**, and it does establish it:

1. the revealed 32 bytes hash (WebCrypto SHA-256, in this tab) to the commitment the table published
   for this hand;
2. all seven cards the player saw sit at exactly the deck positions that seed and the published
   shuffle rule put them at;
3. therefore the deck was **fully determined by the commitment** — change one card and (1) breaks.

**What it does not establish** is *when* the commitment came into existence. The panel prints
`proof.timestamp`, a `nat64` the table canister wrote about itself, and renders the word "before".
A canister that derived the seed after seeing hole cards and back-dated that field would produce a
page identical in every pixel. The browser witnessed a hash, not an ordering.

So the claim is demoted to **Not proven**, and the panel now says what would settle it: the
commitment is on screen from the moment cards are dealt, so a player who copies it mid-hand and
compares it after the reveal has witnessed the ordering themselves. That is a client-side path to
the property, and it is one screenshot of work. Two other statements were tightened with it: the
tally's "Nobody could have chosen them after seeing your hand" became "Change any one of them and
the hash in step 3 stops matching", and rung 1's "Published at X, before a single card was dealt"
now attributes that timestamp to the canister and names it as the one thing on the panel taken on
trust.

`docs/SHUFFLE-SPEC.md:21` carries the same sentence in its **Proven** row and is not owned by this
pass. It should be demoted the same way.

#### The verification really is local, measured

With every canister request aborted at the browser (37 attempted, 37 failed) and the panel's own
"Run the check again" clicked: the headline came back `.headline.good`, the tally came back
"7 of 7 cards", and the derived hole cards `2♦ 8♣` were byte-identical to the online derivation.
The tell that this was a fresh, networkless run rather than stale DOM is the canister echo under
"Show the work", which flipped from *"true — proves only that the canister can hash"* to
*"unreachable — and the check above still passed without it"* in the same pass. The page said
"I cannot reach the table" and "verified" at the same moment.

---

## 4. Making it one product

`index.scss` was a **57-line reset with no tokens**. Everything visual lived in thirteen
per-component `<style>` blocks written by four people who could not see each other's work. Counted
across them:

| axis | distinct values in the tree | unified to |
|---|---|---|
| `border-radius` | **16** (2 3 4 5 6 7 8 9 10 11 12 14 16 18 20 999) | 4: `--cd-radius-chip 8` / `-card 12` / `-panel 16` / `-pill 999` |
| `font-size` | **28** (8 → 44 px, incl. 8.5 9.5 10.5 11.5 12.5 13.5 14.5) | 6: `--cd-text-xs 11` `-sm 13` `-md 14` `-lg 18` `-xl 24` `-display 34` |
| font-weight | 400 500 600 **650** 700 800 | 4: body/medium/strong/figure = 400/500/600/700 |
| "our" green | **9** hexes | 1: `--cd-accent #00d4aa` |
| "our" amber | **6** hexes | 1: `--cd-money #fbbf24` |
| muted grey | **11** hexes, no scale | 4: `--cd-ink` / `-ink-1` / `-ink-2` / `-ink-3` |
| panel tint | **8** `rgba(255,255,255,0.0x)` | 3: `--cd-surface-1/2/3` |
| panel stroke | **8** | 3: `--cd-line` / `-soft` / `-strong` |
| spacing | 2 3 4 5 6 7 8 9 10 11 12 14 16 20 … | 4px grid, `--cd-space-1…6` |
| money format | 7 private copies of ÷1e8 | `formatTokenAmount()` — see the caveat below |

**Nine greens.** `PokerTable.svelte` does not use the brand teal at all: its positives are
`#49a16e` and `#7ee2b8`. So the felt and the header are two different products' greens, side by
side, in every screenshot. The fairness verdict is a third (`#2ecc71`) and the hashes a fourth
(`#4ecdc4`) — four greens in the one screenshot whose subject is trust.

**The clearest single example, measured with `getComputedStyle` on four buttons in ONE header row:**

| button | font | radius | height | accent |
|---|---|---|---|---|
| `Lobby` | 14px/400 | 10 px | 42 px | white 5% |
| `History` | 13px/500 | 10 px | 38 px | **indigo** `rgb(99,102,241)` |
| `Verify Fair` | 13px/500 | 10 px | 38 px | **teal** `rgb(0,212,170)` |
| `ShadowDragon71` | 14px/500 | **8 px** | 46 px | teal |

Three heights, two radii, two sizes, two weights and **two accent families**, for four controls
that all mean "open a thing". Indigo is a whole second accent family (`#6366f1` `#818cf8` `#a5b4fc`)
that arrived with the deposit modal and appears in five components meaning five different things.

Dialog chrome: panel radius **20 px** (deposit) / **16 px** (fairness) / **14 px** (lobby panes);
title **18px/700** (deposit) vs **16px/650** (hand replay).

### What was actually changed, and what was not

**Changed (all verified live against the real canisters):**

| # | change | before → after, measured |
|---|---|---|
| T-11 | table header quotes the table contract, not the lobby's stale record | `6-Max - 0.01/0.02` → `6-Max · 0.05/0.10`, chain `5_000_000/10_000_000` |
| T-11 | that pill is now a **gated** money figure (`dom-scrape` + `chain-agreement` + a `headerstakes` fault-injection target) | pre-flop 8 → **10** money figures; facing-bet 14 → **16**; injected drift goes UNVERIFIED with two named disagreements |
| T-10 | the action-feed key no longer touches JSON | **19 → 0** uncaught page errors per hand; action log **0 → 3** entries, with amounts and streets |
| T-17 | the hand-export button no longer throws on BigInt | same class, different control |
| T-18 | the withdrawal receipt states the real amounts instead of the ledger block index | withdrawing 1 ICP printed **"0.0000 ICP sent to your wallet"** (block 1130 formatted as money) → `Withdrew 1.0000 ICP. 0.9999 ICP reached your wallet after the 0.0001 ICP network fee. Ledger block #1130.`, escrow delta `100_000_000` e8s |
| T-12 | the **main** pot is labelled `Main`, not `Side 1` | `SIDE 1 0.40` under `TOTAL POT 0.40` → `MAIN 0.40` |
| T-13 | one dismissal contract for all four dialogs | Escape closed 1 of 4 → **4 of 4** |
| D-03 | `index.scss` carries the token vocabulary, each token set to the **majority existing value** on its axis | 57 lines → tokens + one global rule |
| D-03 | money is `tabular-nums` everywhere, keyed off the class names components already use | `11.90` and `50.00` now align in adjacent pods |

**Deliberately not changed, and why:**

* **Thirteen component stylesheets were not rewritten to use the tokens.** That is a redesign, and
  the brief said not to start one. The vocabulary is written down in the one file every surface
  imports, so wave 4 has somewhere to point.
* **The lobby ROW NAME still lies** (`9-Max - 0.01/0.02` on a 0.10/0.20 table). Fixing it in the
  client would turn the only red scene green and hide a live backend defect. The fix belongs in
  `init_microstakes_tables`, which hardcodes table_1's config for all three tables.
* **The duplicated side-pot figure** is not suppressed, because `chain-agreement.mjs` asserts
  `dom.sidePots.length === truth.sidePots.length` and hiding the single-layer case would fail
  another agent's gate.
* **The second disclaimer copy** was not deleted. Rule 2, lead's call.
* **`VITE_LOCAL_GATEWAY_PORT`** was not set, because doing so makes the harness's page origin and
  agent origin differ and the shim's "same origin, no CORS" property is what makes runs work today.
  It is a harness change, not a one-liner.

---

## 5. Where the look and feel actually stands, scene by scene

One tool, the hue-mask/largest-component measurer from DESIGN-BAR §1.1, pointed at our captures and
the reference corpus in the same run.

### Desktop table — **competitive, and the aspect-ratio finding is real**

| capture | window | felt | aspect | width % | **area %** |
|---|---|---|---|---|---|
| ClearDeck `table-preflop` | 1440×900 | 962×452 | **2.13** | 66.8% | **33.6%** |
| ClearDeck `table-allin` | 1440×900 | 980×463 | **2.12** | 68.1% | **35.0%** |
| ClearDeck `table-empty` | 1440×900 | 984×505 | 1.95 | 68.3% | 38.3% |
| PokerNow | 1512×945 | 957×474 | 2.02 | 63.3% | 31.7% |
| PokerStars (green) | 670×577 | 558×262 | 2.13 | 83.3% | 37.8% |
| GGPoker | 900×632 | 760×341 | 2.23 | 84.4% | 45.6% |
| WPT Global `tphB-2` | 1122×754 | 848×408 | 2.08 | 75.6% | 40.9% |
| WPT Global `beasts-1` | 1440×1000 | 1058×402 | 2.63 | 73.5% | 29.5% |
| WPT Global `beasts-3` | 1439×994 | 1058×401 | 2.64 | 73.5% | 29.7% |

Wave 2 measured ClearDeck at **1.84 and a rounded rectangle**. It is now **2.12–2.13 and a true
ellipse** (span-profile fill 88.3%), inside the reference band of 2.02–2.64. On **area** — which is
what a player perceives — ClearDeck is **33.6–38.3% against a reference median of 34.75%**, i.e.
above the median. The table-redesign agent nominated "the felt is only 66.8% of window width against
a reference median of 73.5%" as its biggest gap; that is true in width and **wrong as a conclusion**,
because our window is wider than every reference window except WPT beasts, and area does not follow
it. Its own critic caught this and was right.

**Verdict: ours.** Against PokerNow specifically: wider felt share, equal ellipse quality, and a
chain-checkable pot decomposition (`0.00 collected + 0.30 betting`, asserted to sum to `get_pot()`)
where PokerNow shows a bare number.

### Desktop all-in — **theirs, and one of the reasons is a money bug**

GGPoker puts a live equity badge and a named hand-strength readout on the felt at the all-in moment.
We show neither. Worse, our only quantitative content there is the pot-odds strip and it reads
`POT ODDS 1.6:1 · Call 69.80 to win 110.20 · need 39%` for a hero whose own pod, three inches away,
reads `24.80`. **The hero cannot put in 69.80.** Calling all-in for 24.80 makes a 75.00 main pot, so
break-even is 24.80/75.00 = 33.1% and the maximum winnable is 75.00.

The screenshot gate certifies it, because `get_table_view` computes `call_amount` as
`state.current_bet - player.current_bet` with **no clamp** (`lib.rs:5195`) while the execution path
clamps with `.min(player_chips)` (`lib.rs:3405`). The screen agrees with the chain and both are
wrong. Agreement with the chain is being used as a stand-in for correctness, and here they part
company. **This is a wave-4 engine fix, not a client fix.**

### Desktop showdown — **split**

Ours states the transfer explicitly (`You won 24.00 ICP / TWO PAIR / COMPLETE`) and tags the winning
two cards in situ, against GGPoker's `Total Pot : $3,306` with no payee. But PokerNow puts a `+450`
delta chip **under the winner's pod**, where the money landed; ours puts the amount in a banner
300 px away, and the pod shows only the post-hand stack.

### Desktop side pots — **ours, uniquely**

We render `TOTAL POT 155.00` plus `Main 80.00` and `Side 1 75.00`, each asserted against an element
of the canister's `side_pots` vector. **No capture in the corpus, from any of the four clients,
shows per-side-pot amounts on the felt at all.** WPT shows `Total pot 34.3` and nothing else.

### Fairness proof — **ours, and nothing in the corpus competes**

52 deck slots each labelled with its integer index; seven dealt cards each tagged with a deck
position and a tick; computed-vs-committed hashes as two 64-char rows; the first three shuffle steps
printed with chain hex, first-8-bytes, draw value and `swap deck[i] <-> deck[j]`; copy-paste commands
for two in-repo reference verifiers; and the canister's own `verify_shuffle` demoted to a labelled
echo that says in the UI that its answer is worth nothing. PokerNow's Session Log offers a download
button and no seed. **This is the best thing in the product.** Its problem is that the gate does not
check that it ran (H-24).

### Hand history — **theirs on density, ours on legibility**

PokerCraft's row carries 7 sortable/filterable fields with mini card art; ours carries 5 at a 60 px
pitch with no sort and no filter beyond two scope chips. Ours adds a per-hand "7 cards re-derived
here" badge that GG has not got. On contrast our weakest element is 3.71:1 against PokerNow's 1.76:1
timestamps, and our colour-coded verbs clear AAA where PokerNow's `folds` fails AA at 4.00:1.

### Lobby — **split, and the split is structural**

The preview pane genuinely beats WPT Global **18 data fields to 4**, including a deck commitment I
verified character-for-character against `get_table_view()`. Signed out, we show **3 tables with
live boards and copyable canister ids** against GGPoker's **0**. But the row itself supports 3 of
the 6 canonical lobby decisions against PokerStars' 6, and **you cannot use any of it until you have
scrolled**: first row at 88.4% of the desktop viewport against PokerNow's 36.1%, and at 119% of the
mobile viewport — zero rows, ever, before a scroll.

### Mobile — **theirs, by 2.5×. This is the wave-4 headline.**

| capture (390×844) | felt | width % | height % | **area %** |
|---|---|---|---|---|
| **PokerNow portrait** | 313×548 | 80.3% | 64.9% | **52.1%** |
| ClearDeck `table-showdown` | 217×319 | 55.6% | 37.8% | **21.1%** |
| ClearDeck `table-empty` | 217×316 | 55.6% | 37.5% | **20.9%** |
| ClearDeck `table-allin` | 217×299 | 55.6% | 35.5% | **19.7%** |

Ours is **38–40% of PokerNow's playing surface on the identical device.** Text follows the surface:
our mobile stack digits measure 8–10 px against PokerNow's 15 px.

And two of six mobile table captures **do not contain a poker table**: in
`table-preflop-mobile.png` and `table-facing-bet-mobile.png` the pot, the whole board and four of
six pods are above the top of the frame (the measurer reads `tallest_col_rel = 0.000`, the mask
flush against the top edge) with ~215 px of empty space below the dock. Both were filed VERIFIED,
because **the DOM assertions never look at the picture**. `scrapeTable` reads `textContent` with no
visibility, bounding-box or in-viewport test.

---

## 6. Claims from the builders that did not survive

| claim | what I found |
|---|---|
| "The lobby-versus-canister stakes disagreement is the only unverified scene left" | The same wrong stakes string was in the **header of every table capture**, and 18 table scenes were filed "agrees with chain: yes" around it. Now fixed and gated (T-11). |
| "All six table scenes verify at mobile" (offered as evidence the reflow works) | True of the DOM, false of the pictures. Two of six mobile PNGs do not show a table (T-16). |
| "Desktop full-page 3200 px → 2320 px" | Not reproducible. `document.documentElement.scrollHeight` on the signed-out lobby is **1728**. The direction is right, the figure is not. |
| "The rake line reads: all 155.00 ICP of the last pot went to the winner" | Rendered on a hand still in progress: `pot = 15_500_000_000`, `phase = Flop`, `hand_number = 1`, no pot had been paid. An unsettled pot presented as a completed payout, on the flagship proof line. |
| "The single biggest gap is the disclaimer chrome / felt width share" | Desktop felt **area** is above the reference median (33.6–38.3% vs 34.75%). The gap is mobile, at 2.5×. |
| "The fairness panel never auto-verifies; 0 of 7 trials" | Not what I see. On a quiet completed hand it verifies **untouched in 505 ms, 0 page errors**, in both arms of a controlled A/B. The real defect is H-24: the gate does not assert the verdict, and it accepted a dead panel and a live one in consecutive runs. |
| "210 money figures asserted" | The filed artifact summed to 209. It now sums to **244**, which is a different run and a different assertion set; the lesson stands — quote the artifact, not the memory. |

---

## 7. Wave 4, ranked

**Is the engine safe to leave alone? Almost — with one exception, and it is not a small one.**

The money *destination* question is closed by construction and I could not shake it: `principal_of`
is deleted, the owner travels with the stake, `apply_payouts` credits a `Principal` and not a seat,
M8 runs in the fast gate, the settlement oracle carries a per-principal column, and 6 of 7 seam
mutations die. The upgrade blockers are closed and M7 walks real `801aa79` state — escrow, seated
stacks, hole cards, board, deck cursor, shuffle commitment, anti-replay record, a live flop —
through the boundary intact. **I would not reopen the payout path.** But E-40 is an engine defect
that can abort a settlement, and the all-in `call_amount` clamp is an engine defect that makes the
client tell a player to bet money they do not have. Both are engine work.

### Do these first

1. **E-40 — guard the third `evaluate_hand` call site** (`lib.rs:871`), exactly like its two
   siblings. A trap in a hand-history writer can wedge a settlement. Then make the fuzzer reach it
   and pin the sequence. *Half a day for the guard, days for the reachability proof.*
2. **`call_amount` is unclamped in `get_table_view`** (`lib.rs:5195` vs `lib.rs:3405`). The client
   currently offers a hero holding 24.80 a call of 69.80 and quotes a required equity 6 points
   wrong, and the gate certifies it because the canister is wrong the same way. Clamp it, and add a
   money-safety invariant that `call_amount <= player.chips`.
3. **T-14 — make the app open in a browser.** Set `VITE_LOCAL_GATEWAY_PORT` and move `run.mjs` to
   serve the page from the gateway origin so the shim can be deleted. Until this lands, no
   screenshot in this repo is evidence about what a user sees. Also: never print a bundle stack
   trace to a player, and stop blaming the canister for a failed fetch.
4. **T-16 — the phone.** 19.7% of the screen against PokerNow's 52.1% is not a polish item, it is
   the difference between a game and a web page with a picture of a table on it. The cheap half is
   the duplicated 235 px disclaimer block (lead's call under rule 2); the rest is a real reflow.
   Fix the two mobile captures that do not show a table, and make the harness fail a scene whose
   felt is not wholly inside the viewport.
5. **H-24 — assert the verdict, not the markup.** Wait for `.headline.good` and the "N of N cards"
   tally, and add a flipped-seed negative control. The best feature in the product currently ships
   behind a gate that cannot tell whether it ran.

### Then

6. **The lobby canister's hardcoded configs.** `init_microstakes_tables` writes table_1's blinds
   into all three records and bakes them into the name. This is the root of T-11 and of the only
   red screenshot scene. Take the configs as arguments or read them from the tables.
7. **The lobby's first row at 88.4% / 119%.** ~345 px of heading, filters and drift banner sit
   between a category-normal hero and the first choice. Every reference client is between 19.9% and
   42.6%.
8. **D-03 — adopt the tokens.** Thirteen stylesheets, one axis at a time, colour first (nine greens
   → one), then radius, then type. Add a CI check that fails on a hex colour or a raw `border-radius`
   in a component `<style>` block; otherwise the ninth green comes back.
9. **T-15 — state the money-precision rule and adopt one formatter.** Two documented precisions
   (felt/lobby 2 dp, wallet/fee 4 dp), not one function pretending there is only one.
10. **H-25 — give the open engine defects markers.** One marker for the lowest-severity entry in the
    register is a green tick over an empty set.
11. **The invariant scope gap the engine agent's own critic named.** M8 convicts the hand shape it
    was written from; a pot share paid to the wrong *live* claimant passes all three tests. Wire
    `check_no_free_money` into the fuzzer's per-step loop (reachability) **and** run
    `check_principal_attribution` on ordinary showdowns using the settlement oracle's own
    expectation (scope). Both halves, or the class stays covered only where somebody scripted it.
12. **H-23 — CI runs none of the fund-safety harnesses.** `./scripts/dev.sh test` now runs them all;
    the CI change is one job that calls it.
13. **Deposit and sit-down have no confirmation.** A client custodying real ICP that moves 12 ICP
    and says nothing, and seats you for an unspecified buy-in, is not finished.

### Which screens still lose, and by how much

| screen | verdict | margin |
|---|---|---|
| Mobile table, any scene | **loses** | playing surface **19.7–21.1%** of screen vs PokerNow **52.1%** — 2.5× |
| Lobby, first choice | **loses** | first row at **88.4%** desktop / **119%** mobile vs 26.6–36.5% |
| All-in moment | **loses** | no equity readout at all, and the one number shown is unofferable |
| Hand-history list | **loses** | 5 fields vs PokerCraft's 7, no sort, no filter |
| Desktop table geometry | **wins** | aspect 2.12–2.13 and area 33.6–38.3% vs a reference median of 34.75% |
| Side pots on the felt | **wins** | decomposed and chain-asserted; **no reference client shows this at all** |
| Signed-out lobby | **wins** | 3 live tables + 18 preview facts vs GGPoker's 0 |
| Fairness proof | **wins, uncontested** | nothing in the corpus ships an equivalent surface |
| Hand-history legibility | **wins** | weakest element 3.71:1 vs 1.76:1 |
