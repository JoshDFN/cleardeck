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
```

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
still leaves forensic evidence.

## Scenes

| scene | what it must show | how the state is reached |
| --- | --- | --- |
| `lobby` | table list as a new visitor sees it | signed out; rows and player counts are live lobby + table canister reads |
| `table-empty` | table with open seats (README state) | `table_3` reset to empty, hero signed in but not seated |
| `table-preflop` | seated, cards dealt, action on us, bet controls live | `table_2`, hero + opponent buy in, hand started, opponent calls, stop when `action_on == hero && phase == PreFlop` |
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
* Third-party assets (Google Fonts, dicebear avatars, the CoinGecko price ticker) are cached
  to `artifacts/screens/.thirdparty-cache/` on first run and replayed afterwards. This is
  the only interception in the harness, it covers **non-canister** assets only, and it makes
  runs both repeatable and offline-capable.

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
