# ClearDeck screenshot harness

Deterministic, re-runnable screenshots of the **real** ClearDeck app in **real** on-chain
game states, for look-and-feel A/B judgement.

```bash
node tools/shots/run.mjs
```

That single command builds the frontend with the local canister IDs wired in, deploys the
asset canister to the local replica, drives real hands through the real table canisters,
and writes PNGs for every scene at desktop and mobile.

Nothing in here mocks a canister response or fakes UI state. Every card, pot, side pot,
seed hash and winner in a screenshot came out of a canister on the local replica.

---

## Requirements

* The shared **managed local network** for this project must be up: gateway on
  `http://localhost:8077` (port pinned in `icp.yaml`). The harness **refuses to start or
  restart it** — other agents share the replica. If the gateway is not answering, step
  `[1/6]` fails with an explicit message.
* The local **ICP ledger** must be live at `ryjl3-tyaaa-aaaaa-aaaba-cai` (the ID the table
  canister hardcodes). That is what makes the real deposit path work locally with zero code
  changes.
* The local throwaway identities `cd-alice`, `cd-bob`, `cd-carol`, `cd-local-deployer` must
  exist in `icp identity list`, funded with local ICP. `cd-local-deployer` must be a
  controller of the table canisters (it is the deployer).
* `npm install` inside `tools/shots` (pulls `playwright@1.61.1`, which matches the cached
  `chromium-1228` in `~/Library/Caches/ms-playwright` — no browser download needed).

## Usage

```bash
node tools/shots/run.mjs                              # all 9 scenes, both viewports
node tools/shots/run.mjs --scenes lobby,table-showdown
node tools/shots/run.mjs --viewports desktop
node tools/shots/run.mjs --skip-build --skip-deploy    # reuse the current dist + deploy
SHOTS_DEBUG=1 node tools/shots/run.mjs                # stack traces on fatal errors
SHOTS_PRICE_FIXTURE=1 node tools/shots/run.mjs        # offline: labelled placeholder prices

node tools/shots/test-census.mjs                      # the inverted gate's own self-check
node tools/shots/test-money.mjs                       # the money parser's own self-check
```

> `SHOTS_CENSUS=report` **turns the inverted gate off** (see below): the token census still
> runs and still reports, but an unaccounted-for number no longer fails the scene. It exists
> for the first pass after a redesign, when the enumeration is being read rather than enforced.
> A run that used it says so in the scene notes (`REPORT ONLY, NOT GATING`) and in
> `manifest.json` (`checks.tokenCensus.mode`). Never file evidence from such a run without
> quoting that line.

**Finish with a full run.** A partial run rewrites `latest/manifest.json` and
`latest/INDEX.md` to describe only the scenes it captured, while the other scenes' PNGs stay
in `latest/`. The images are still correctly named, but the index beside them then
under-reports the directory. Treat `latest/` as authoritative only after a full run.

Exit code `0` = every scene captured **and** verified, `1` = at least one scene did not
verify, `2` = the run could not start (replica down, build not wired, port in use…).

## Output

```
artifacts/screens/<git-short-sha>/
  <scene>-desktop.png          exactly 1440x900
  <scene>-desktop-full.png     full page (ClearDeck pages are taller than any viewport)
  <scene>-mobile.png           exactly  390x844
  <scene>-mobile-full.png
  motion/table-showdown/
    table-showdown-allin-to-showdown.webm    Playwright video of the transition
    frames-desktop/frame-000.png …           frame burst through the same transition
  manifest.json                machine-readable: per-scene on-chain state + DOM checks
  INDEX.md                     human-readable table of every shot
artifacts/screens/latest/      stable mirror of the newest full run
artifacts/screens/.thirdparty-cache/   cached fonts/avatars (see "Determinism")
```

`FAILED-<scene>-<viewport>.png` is written instead when a scene throws, so a broken scene
still leaves forensic evidence. `UNVERIFIED-<scene>-<viewport>.png` is written when the page
rendered fine but the scene's own `verify()` returned false — the canonical filename is
reserved for state that actually passed its assertions, because that filename is what a
reader treats as proof. Every variant of a (scene, viewport) is cleared from `latest/`
before a run writes, so a verified PNG from an earlier commit cannot survive next to this
run's `UNVERIFIED-` one.

## The controller identity is resolved, not assumed

Controller-only calls (`reset_table`, which is what makes a run idempotent) used to be
signed as a hardcoded `cd-local-deployer`. That is an assumption, and on this machine it is
false: `./scripts/dev.sh local-up` runs `icp deploy` with no `--identity`, so the canisters
end up controlled by whatever identity happens to be the machine's **current default** —
which may belong to an entirely unrelated project. `resolveControllerIdentity()` in
`lib/ids.mjs` reads the real controller list off a local canister with
`icp canister status` and picks a local identity that is actually in it — preferring
`CONTROLLER_IDENTITY`, then the funders, then **every name in `icp identity list`**, then the
current default — and fails with an explicit message listing what it tried if none qualifies.
The identity actually used is recorded in the manifest as `controllerIdentity`.

That name enumeration is the wave-7 half of the fix (docs/DEFECTS.md E-53). Resolving from a
four-name allowlist plus whatever was selected was still an assumption: on this machine the
tables are controlled by `cyclepay-hotwallet`, none of the four, and the entire sweep aborted
with `No local icp identity controls 46el7-…`. Since the pixel gate, the occlusion gate and
the protected-notice gate all live behind this sweep, one stale name list disabled three
fund-adjacent gates at once and reported it as a setup error rather than a red gate.

**The frontend deploy needs the same treatment, and did not have it.** `deployFrontend()` in
`lib/frontend-build.mjs` ran `icp deploy -e local frontend` with no identity at all, and
`icp deploy` starts with a controller-only `update_settings` call. On this machine that is a
third identity again — the asset canister belongs to `oms-port-trial` — so `local-up` died in
step `[6/6]` with `IC0512` *after* a successful backend deploy. It now resolves the controller
the same way, and skips the lookup when the canister does not exist yet, because a canister
that has never been created has no controller list and the deploy is what creates it.

## An aborted run does not delete the previous run's evidence

`latest/` is wiped by a full run, because a verified PNG left over from an earlier commit is
exactly the artifact a reader would trust. That wipe is **lazy**: `run.mjs` hands
`claimLatestDir` to the scene runner and it fires from `writeShot`, one statement before the
first PNG is written. It used to run in the preamble of step `[6/6]`, before the first scene,
and the identity check above fires inside the scenes — so a run that captured nothing still
deleted `INDEX.md`, `manifest.json` and every PNG, and a wave-6 reconciler had to rebuild
`latest/` by hand out of two older run directories.

## Every scene is also measured with an error toast up

HARD RULE 2 says the four protection notices and the no-rake property are on screen and
legible **at any viewport on any view**. `lib/protected-notices.mjs` measures that on the
rendered pixels — and it was green throughout a wave in which any error covered all five at
portrait (docs/DEFECTS.md E-52), because no scene in this harness had ever raised one. The
notices were measured on the rendered page; the *set of rendered pages* was the blind spot.

So `lib/toast-notices.mjs` runs centrally, for every scene at every viewport, right where the
census and the pixel gate run: it raises a toast, re-runs the notice probe with it up, and
asserts three separate things — the hit test, that the toast's box fits inside the viewport,
and that it starts at or below the bottom of the notice banner. The toast it raises carries the
component's own Svelte scope class and must `matches()` one of the `.toast` rules in the live
CSSOM, so it is painted by the app's own rule rather than by a lookalike. The `toast-notices`
scenario then raises a **real** one through the app's own error path and compares the two,
property by property, so "the injected node is the thing the player sees" is a measurement
rather than a promise.

## What a scene asserts: agreement with the chain, not presence of an element

A scene is verified when **every money figure on the screen equals the number the canister
reports for it**, checked at the precision the client chose to display. Presence checks still
run — they are what tells a reader the scene photographed the thing it claims to — but they
can no longer carry a scene on their own.

`lib/chain-agreement.mjs` is the assertion; `lib/dom-scrape.mjs` is the only place that knows
a CSS selector belonging to another agent's component; `lib/money.mjs` decodes a displayed
figure back into the interval of chain values that could have produced it.

### The coverage table

Being explicit about what is **not** covered matters as much as what is: a reader who assumes
a surface is gated when it is not is worse off than one who knows it is open.

| surface on screen | asserted against | where |
| --- | --- | --- |
| headline pot | `get_pot()` | every table scene |
| pot breakdown ("X collected + Y betting") | sums to `get_pot()`; the second leg is `sum(current_bet)` | table scenes that render it |
| each seat's stack | that seat's `chips` in `get_table_view()` | every table scene |
| each seat's live bet | that seat's `current_bet` (absent iff zero) | every table scene |
| each side pot, and the count | `side_pots[i].amount`, in order; also that they sum to `get_pot()` | `table-sidepots` |
| the board | `get_community_cards()`, rank and suit, in order | every table scene |
| pot-odds ratio | `get_pot() / call_amount` | `table-facing-bet` |
| required-equity % | `call / (get_pot() + call)` | `table-facing-bet` |
| "Call X" on the button and in the turn hint | `call_amount` | `table-facing-bet` |
| **½ Pot / Pot bet presets** — the amount that would be **sent** | `current_bet + get_pot()(/2) + call_amount`, plus the implied pot | `table-facing-bet` |
| wallet / table balance | `get_balance()` for the signed-in principal | table scenes, deposit |
| winner banner amount and seat | `last_hand_winners[..]` | `table-showdown` |
| lobby blinds and buy-in range | the **TABLE** canister's config, with the lobby's own record cross-checked | `lobby` |
| lobby **row name** ("9-Max - 0.01/0.02") | the TABLE canister's blinds — the name is the biggest money figure on the row | `lobby` |
| lobby live pot per row | `get_pot()` on that table | `lobby` |
| deposit modal wallet balance | ledger `icrc1_balance_of` | `deposit` |
| deposit modal fiat value | balance × the quote actually served this run | `deposit` |
| hand-history row pot | history canister `total_pot`, cross-checked against the table's `sum(winners.amount)` | `handhistory` |
| **no rake** | history canister `rake == 0`, per hand | `handhistory` |

| lobby **preview pane**: heading blinds, mini-felt live pot, each seated stack, the facts list (blinds, buy-in, ante, clock, hands dealt, last pot) and the rake line's pot | the TABLE canister's config and live view | `lobby` |
| deposit modal "Minimum deposit" / "Network fee" | the minimum the table canister enforces, and the ledger's own `icrc1_fee()` | `deposit` |
| **every money, equity and card figure: that nothing is painted over it** | the rendered PIXELS, by a four-shot differential per (figure, occluder) — not the DOM | every scene, both viewports |

### The denominator: every number on the screen is counted

The table above is a **numerator**. Until wave 4 there was no denominator, and that is not a
theoretical complaint: the wave-3 critic rewrote `.current-table-name` — the largest teal
string on every table scene, quoting the blinds — to `9.99/19.98`, and the run still reported
*"14 money figures on screen all equal the canister's"* and filed the canonical PNG. A money
surface nobody thought to assert was invisible in a green run.

`lib/token-census.mjs` inverts the question. After every scene has had its say, **every
numeric-looking token the page renders** is enumerated and each one must be either

1. **matched to a figure** the chain-agreement layer really compared with a canister value
   (one-to-one: three seats showing `12.00` need three distinct passing checks), or
2. **declared non-monetary** in `token-allowlist.mjs`, which is small, in-repo and reviewed.

Anything else is `UNASSERTED` and **fails the scene**. The count is in every run's
`INDEX.md` (`tokens on screen / chain-matched / allowlisted / UNASSERTED`) and the full
enumeration, including every allowlist rule and how many tokens it excused, is in
`manifest.json` under `checks.tokenCensus`.

The allowlist is the only way this can be defeated, so it defends itself:

* every rule needs a **selector**, a **token pattern**, and a reason; the census throws on a
  malformed rule rather than excusing everything (its first draft did exactly that —
  `new RegExp(undefined)` is `/(?:)/`, which matches every string);
* **the shape invariant**: every amount this client renders goes through a `toFixed`, so a
  money figure always carries a decimal point. A rule whose pattern accepts a decimal token
  must be marked `moneyShaped: true`, and the census refuses to run otherwise. Exactly one
  rule is (`.version`, for `v0.1.0-alpha`);
* chain checks are consulted **first**, so a rule can only excuse a leftover token in an
  element, never shadow a check that would have failed;
* `node tools/shots/test-census.mjs` is the offline self-check: 16 cases, no replica needed,
  including "an unasserted money token fails the scene" and "a money-shaped token in an
  allowlisted element is refused".

**Not asserted today** — open surfaces, listed so nobody mistakes silence for a pass. Note
that "not asserted" now means "fails the census if it is ever on screen", not "invisible":

* the **action log** (`ActionFeed`) bet/raise/win amounts — the drawer is closed in every
  scene, so no scene reaches them;
* the **withdraw modal** — no scene opens it (and see `docs/DEFECTS.md` for what is wrong
  behind it);
* the header wallet balance in `WalletButton` as distinct from the table wallet panel;
* the hand-history **detail** panel (per-winner amounts); only the row pot is checked;
* everything on a **BTC** table: `btc_table_1` is not registered in the lobby (T-05), so no
  scene can reach it, and the sats formatting path is therefore unexercised.

### What COVERS what: the pixel gate

Everything above reads `textContent`. None of it can see the only question a player's eye
asks — **is the figure on screen, or is something on top of it**. Two defects proved that gap
is not theoretical. Both were photographed by this harness and filed as **verified**, because
the text was right:

* `docs/DEFECTS.md` **T-22** — on a phone the winner's `100.00%` equity badge rendered as a
  visible `0%`; the hero's own card covered the rest of it. The badge sat inside
  `.player-nameplate` (`z-index: 6`, a stacking context) while the cards paint at `z-index: 7`
  outside it, so no z-index the badge chose could win.
* `docs/DEFECTS.md` **T-23** — the `+24.00` award chip covered the winner's revealed pair and,
  on desktop, a community card's suit pip: the cards that justify the award.

`lib/occlusion.mjs` runs centrally in `run.mjs`, like the census, so no scene can forget it. It
enumerates every **money, equity or card figure** on screen and requires that nothing paints
over it. A finding has to survive three stages:

1. **Geometry** — the candidate's client rect intersects the figure's, inside the visible
   viewport. Necessary, never sufficient: most intersections are containers.
2. **Effective paint order** — CSS 2.1 Appendix E is simulated over the real computed styles
   (stacking contexts from `transform`, `opacity`, `filter`, `contain`, `container-type`,
   `position: fixed/sticky`, positioned-with-a-z-index, flex/grid items with one; the *pseudo*
   stacking contexts that positioned `z-index: auto` elements form, whose positioned
   descendants escape to the parent context; and the tree-order layers). Every element gets one
   integer, so "is above" is one comparison that is right across contexts. **A naive z-index
   comparison is not good enough in either direction** and the manifest proves it per scene:
   across the full 22-shot sweep a naive gate would have raised **3,836** flags and an
   effective-paint-order-plus-geometry gate **1,487**, all of them false.
3. **Occlusion evidence**, which is what actually gates:
   * **hit testing** — `elementFromPoint` on a 5×3 grid across the figure, recorded per point;
   * **a four-shot pixel differential** per (figure, occluder): `A` both visible, `B` occluder
     hidden, `C` figure hidden, `D` both hidden, all clipped to the figure's rect. The figure
     paints ink at a pixel when hiding it changes that pixel (`|B−D|`), and it is **covered**
     there when the occluder has suppressed at least 75% of that contribution (`|A−C|`).
     Everything is hidden with `visibility: hidden`, which paints nothing and moves nothing, so
     no reflow can contaminate the comparison.

The pixel differential is the **authority**, and hit testing is corroboration, because hit
testing cannot see an occluder with `pointer-events: none` — `.board-cluster` is exactly that
in portrait — and because a rect intersection that covers only padding covers no ink at all.

**Thresholds, and why they are defensible.** The fraction is of the figure's **own visible
ink**, never of its box:

| kind | limit | why |
| --- | --- | --- |
| money, equity, a card's rank or pip | **2%** of ink | These are read glyph by glyph, and half a covered glyph is a *different number*: T-22's `100.00%` was read off the screen as `0%`. One glyph of a seven-glyph badge is ~14% of its ink, so a 2% budget cannot hide any part of any digit — while an overlap that only touches padding, a shadow or a rounded corner measures exactly **0.0%**. |
| a card's white face | **20%** of ink | A card is identified by its rank and its pip, and both are gated at 2% in their own right. What is left is a large blank plate: the hero's own bet disc touching the corner of the hero's own card measures 2.8%, and failing that would be a false red on the first scene anyone ran. |

Total occlusion always fails (100% ≥ either limit), and so does a figure on which **no** probe
point is hit-testable, unless the figure itself is `pointer-events: none` and therefore cannot
answer a hit test at all.

**A dialog covering the page behind it is the feature, not the defect.** An occluder inside an
overlay layer (a `position: fixed` ancestor covering ≥40% of the viewport, or anything
`role="dialog"` / `aria-modal="true"`) covering a figure on the page is reported as
`behind-an-overlay`, measured and counted, and does not fail the scene — 186 figures in the
sweep are in that class. The converse is **not** excused: a figure *inside* an overlay is gated
normally, so a dialog that covers its own numbers still fails.

**What the artifact carries.** `INDEX.md` gains two columns — how many figures the page renders
and how many are covered, with the worst offender named inline. `manifest.json` carries, per
finding, the occluded element (path, text, rect, glyph count), the occluder (path, text, rect,
z-index, paint index, whether a naive z-index compare would have agreed), the fraction of ink
and of box, the hit-test answers, and **two crops written beside the PNGs** — the figure as a
player sees it and the same rect with the occluder hidden. It also carries every intersection
the gate *declined* to report with the measurement that cleared it, so a gate that ever starts
firing on decoration shows up there as a near miss first.

**Self-check**: `node tools/shots/test-occlusion.mjs` — 15 cases on constructed fixtures, no
replica and no app build needed. Six are real occlusions that must be caught (including T-22's
exact three-context stacking shape, T-23's chip-over-card shape, an occluder a naive z-index
compare misses, and one with `pointer-events: none`); the rest are overlaps that must **pass**
(padding-only, painted-behind, a translucent veil the figure still shows through, the case a
naive compare gets wrong in the direction of crying wolf, a dialog over the page) plus two
harness invariants: the probe leaves no attribute or stylesheet behind, and a figure on a
**scrolled** page is measured at the right rectangle — Playwright trims `clip` against the
viewport, not the document, and the first draft's document coordinates threw on one scene and
would silently have measured the wrong rectangle on any other scrolled one.

### Proving the assertion has teeth

Two mechanisms, both off by default:

```bash
# 1. FAULT INJECTION. The chain is untouched; the RENDERED number is falsified.
#    Every listed target must actually change something on screen or the scene FAILS —
#    a fault that did not inject proves nothing, and is refused rather than reported.
SHOTS_INJECT_DRIFT=pot,stack,bet,potodds,callbutton \
  node tools/shots/run.mjs --scenes table-facing-bet --viewports desktop
SHOTS_INJECT_DRIFT=board,sidepot \
  node tools/shots/run.mjs --scenes table-sidepots --viewports desktop

# 2. A/B AGAINST A DIFFERENT BUILD, without deploying anything.
#    Static assets come from a build on disk; every /api/* call still goes to the real
#    local replica, so the chain side of the comparison is identical. The manifest records
#    `assetProvenance: LOCAL DISK` so such a run can never be mistaken for the deployed app.
SHOTS_SERVE_DIST=/path/to/other/dist \
  node tools/shots/run.mjs --scenes table-facing-bet --skip-build --skip-deploy
```

`node tools/shots/test-money.mjs` self-checks the figure parser against the shapes the app
really renders, including the case it must refuse: a figure displayed so coarsely that `x` and
`2x` render identically is reported as **blind**, not as agreeing.

### Whose defect is it

Two different failures produce the same red scene and the verdict says which:

* **the client rendered a number the canister does not hold** — a frontend defect;
* **two on-chain sources disagree and the client faithfully renders the authoritative one** —
  a canister-data defect, prefixed `ON-CHAIN DATA DEFECT` and naming the canister to fix.

The `lobby` scene is red today for the second reason and only the second reason: all 16 stakes
and buy-in figures in the cells agree with the TABLE canister, and the four that fail are the
row *names*, strings written once into the lobby canister by `init_microstakes_tables` and
never revised.

## Scenes

| scene | what it must show | how the state is reached |
| --- | --- | --- |
| `lobby` | table list as a new visitor sees it | signed out; rows and player counts are live lobby + table canister reads |
| `table-empty` | table with open seats (README state) | `table_3` reset to empty, hero signed in but not seated |
| `table-preflop` | seated, cards dealt, action on us, bet controls live | `table_2`, hero + opponent buy in, hand started, opponent calls, stop when `action_on == hero && phase == PreFlop` |
| `table-facing-bet` | the decision screen: an amount owed, pot odds, bet sizing | `table_2`, the opponent RAISES every turn, stop with the hero on the clock owing a real call; refuses to pass if `call_amount == 0` |
| `table-allin` | the all-in moment, two players committed | `table_3`, three players go all in in turn order; stop the instant two are all in and the action is pending on the third |
| `table-showdown` | showdown with a winner, winning hand, pot awarded | `table_2` heads-up all-in; the opponent calls `sit_out_next_hand` first so auto-deal cannot start the next hand and the finished hand stays on screen |
| `table-sidepots` | multiple side pots | `table_3` with **four** players (see note below): two short stacks all in, two deep stacks call, hand rests on the flop with side pots on screen |
| `deposit` | the deposit flow / modal | hero seated alone on `table_2`, modal opened from the table wallet panel |
| `handhistory` | hand history with a completed hand | one hand played to completion, then the header History button |
| `shuffleproof` | provably-fair verification for a real hand | same completed hand, then "Verify Fair"; the seed hash and revealed seed are the canister's own |

### Why `table-sidepots` uses four players and not three

The task asked for three players with two all-ins. The engine makes that state impossible
to photograph. With three players, once two are all in only one player can still act, so
`advance_to_next_street` runs the board out and `determine_winners` sets
`phase = HandComplete` and then `state.side_pots.clear()`
(`src/table_canister/src/lib.rs`). `PokerTable.svelte` only renders `.side-pots` while a
hand is in progress — at `HandComplete` the winner banner replaces the pot display — so the
side-pot breakdown is never on screen in a resting state. With four players the two deep
stacks can still act after the short stacks are all in, the hand rests on the flop, and the
real side pots are visible.

## Authentication

The browser signs in through **the app's own local-dev login**: the `Dev Login → Player 1`
button that `WalletButton.svelte` renders whenever the page is not on an IC mainnet
hostname. It calls `auth.devLogin('dev-player-1')`, which derives a deterministic Ed25519
identity (`src/cleardeck_frontend/src/lib/auth.js → createDevIdentity`). Every subsequent
canister call from the page is really signed by that identity against the real canisters —
the app runs its normal authenticated code path, and the principal is stable across runs:

```
dev-player-1  wufmv-ee35x-h6maj-t5vkr-wjebc-xd3cr-uggmi-ytjag-cn5xh-lqkfp-yae
dev-player-2  75qvt-xbg2k-o4v4o-fpveu-whvxu-pxd4d-rhzlv-bdjsr-txn4x-4mjkc-uqe
dev-player-3  ougln-xm7bw-vgre4-zcnzf-z3pga-k4sjf-7q6zv-vovbd-tjm3p-g4smy-lae
dev-player-4  4tgka-ghyhm-4ebbe-3xqm4-alsxm-onnix-mi73y-5bcjg-ecekb-coffw-zqe
```

`lib/identities.mjs` reproduces `createDevIdentity` byte for byte, so the driver can fund
and seat exactly the principal the browser is logged in as.

**Internet Identity is not used, because there is no local II canister on this replica.**
`icp.yaml` declares `ii: true` for the local environment, but the running network was
started without it — `.icp/cache/networks/local/descriptor.json` contains `"ii": false`,
and `rdmx6-jaaaa-aaaaa-aaadq-cai` does not exist locally. Scripted local II is therefore
impossible until the network is restarted with II. If II is added later, the click target to
change is `devLogin()` in `lib/browser.mjs` (swap it for the `Connect Wallet` button plus
the local II flow); nothing else in the harness depends on the auth mechanism.

## The 4943 port shim

`src/cleardeck_frontend/src/lib/ic-config.js` hardcodes

```js
export const LOCAL_HOST = 'http://127.0.0.1:4943';
```

for the agent in local development (and `auth.js` hardcodes the same port for local II),
but this project's gateway is pinned to **8077** in `icp.yaml`. The harness is not allowed
to edit frontend source, so `lib/proxy.mjs` listens on `127.0.0.1:4943` and forwards
verbatim to the gateway, rewriting only the `Host` header:

* `/api/*` and `/_/*` → `Host: localhost:8077` (the gateway resolves the canister from the
  URL path)
* everything else → `Host: <frontend-asset-canister-id>.localhost:8077`, so the page itself
  is served by the **real asset canister**

Serving the page and the agent traffic from the same origin also removes CORS from the
equation. This is a port shim, not a mock: every byte comes from the replica.

**This is a real local-dev defect in the app**, not just a harness inconvenience — a plain
`npm run build && icp deploy -e local frontend` on this project produces an app whose agent
talks to a port nothing is listening on.

## Determinism

* Fixed viewport and `deviceScaleFactor: 1`, so PNGs are exactly 1440x900 / 390x844.
* `colorScheme: 'dark'`, `reducedMotion: 'reduce'`, `locale: 'en-US'`, `timezoneId: 'UTC'`.
* All CSS animations and transitions are neutralised immediately before each still
  (`FREEZE_CSS` in `lib/config.mjs`); `page.screenshot({ animations: 'disabled' })` on top.
* Waits are on **real DOM and real on-chain conditions**, never `sleep`. `waitForState` and
  `playUntil` poll `get_table_view` until the target state is actually true.
* `localStorage` is pre-seeded through the app's own keys (`poker_sound_muted`,
  `poker_avatar_style`, `lobby_view`) so the sound state, avatar style and lobby layout do
  not depend on what a previous run clicked.
* `Math.random` is replaced by a seeded PRNG in the page.
* Live countdowns are landed on a fixed value (`stabilizeTimer` waits until `.turn-timer`
  reads e.g. `40s`) instead of being screenshotted at an arbitrary instant.
* Every scene **resets its table through the controller first**, so `hand_number` is always
  `1` for the hand in the shot — no drift across runs.
* Third-party **static** assets (Google Fonts, dicebear avatars) are cached to
  `artifacts/screens/.thirdparty-cache/` on first run and replayed afterwards. This is the
  only interception in the harness, it covers **non-canister** assets only, and it makes
  runs both repeatable and offline-capable.
* Third-party **volatile facts** are never replayed. `api.coingecko.com` returns a live
  market quote which the app renders as `~$12.34` beside a real on-chain balance; serving a
  cached quote would publish a stale number as a current one, in an artifact whose whole
  purpose is to be evidence. So the price is fetched live and recorded with its timestamp;
  if the fetch fails the harness fulfils **503** so the app shows its own "Failed to fetch
  prices" state (an honest absence, not a stale number); and `SHOTS_PRICE_FIXTURE=1` opts
  into a deterministic placeholder that is labelled `fixture` in both `manifest.json` and
  `INDEX.md`. Any pre-existing cached entry for a volatile host is purged at run start.
  See `volatileThirdParty` in the manifest — that field is the provenance of every fiat
  figure in the run.

Residual, unavoidable variation: the **shuffle seed hash and revealed seed** differ every
run (that is the point of provable fairness), and hole/board cards differ every hand
because the shuffle is real.

## Idempotency and local play money

Each scene calls `releaseSeats` (real `leave_table`, chips back to escrow) and then
`reset_table` as the controller before seating players again, so re-running is safe and does
not slowly burn the local funders. Escrow balances survive `reset_table` — it only
re-initialises `TABLE`/`TABLE_CONFIG` — and buy-ins are topped up from the ledger through
the **real** `icrc2_approve` + `deposit` path when escrow is short. Funding walks
`cd-alice → cd-bob → cd-carol → cd-local-deployer` so one drained throwaway cannot stop a
run.

`table_1` is deliberately left alone (it has a hand in progress with the `cd-*` identities).

## Safety

* `lib/ids.mjs` refuses any `icp` invocation that carries `-e ic` or references a known
  ClearDeck mainnet canister prefix (`kbhpl- kpfcd- kggj7- kieex- lfkaz- lclgn- qrhly-`),
  and refuses to build an actor against such an ID.
* Local canister IDs are read from `.icp/cache/mappings/local.ids.json`;
  `.icp/data/mappings/ic.ids.json` is never read.
* `assertBundleWiredTo` aborts the run if the built bundle does not contain the local lobby
  canister ID — the known trap where a build with no canister IDs looks fine and talks to
  nothing.

## Layout

```
tools/shots/
  run.mjs                  orchestrator (one command does everything)
  lib/config.mjs           all constants: paths, ports, viewports, table configs
  lib/ids.mjs              local id resolution + mainnet refusal + safe `icp` wrapper
  lib/frontend-build.mjs   env-wired vite build, bundle assertion, asset-canister deploy
  lib/proxy.mjs            127.0.0.1:4943 -> gateway port shim
  lib/identities.mjs       the app's own dev-player Ed25519 identities
  lib/agent.mjs            actors built from src/declarations (same IDL the app uses)
  lib/table-driver.mjs     reset / fund / deposit / buy-in / play / waitForState
  lib/browser.mjs          Playwright context, third-party cache, dev login, settle
  lib/capture.mjs          PNG + burst writing, manifest, INDEX.md
  scenarios/               one file per scene, plus _shared.mjs helpers
```

## Repository hygiene

Recommendation for `.gitignore` (not applied here — the lead decides):

```
artifacts/screens/*
!artifacts/screens/latest/
artifacts/screens/.thirdparty-cache/
tools/shots/node_modules/
```

Keeping only `artifacts/screens/latest/` tracked gives reviewers a stable set of images to
diff between waves without committing a new ~40 MB directory per commit. The per-SHA
directories stay local. If image diffs in git history are not wanted at all, ignore
`artifacts/` entirely — every image is reproducible with one command.
