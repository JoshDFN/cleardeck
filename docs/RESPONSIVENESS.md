# Responsiveness, in numbers

How long ClearDeck takes to answer a click, and how long the canister's answer takes to reach
the felt. Every figure below was measured on the real client against the real local canisters,
with the browser's own timing APIs, and every one can be re-derived with a command printed
next to it.

> **Still unaudited alpha software.** Nothing here is a claim about mainnet, where nobody has
> ever played. All of it is localhost.

Two things this document deliberately does **not** do. It does not report a single "fps"
number as though it were a display refresh rate — headless Chromium is not vsync-locked, and
the control measurement below shows exactly how much of the number is the browser rather than
the app. And it does not put ClearDeck in a race against PokerStars, GGPoker or WPT Global:
no gameplay recording of those clients exists at a usable sample rate, so their cells say
NOT MEASURED and stay empty.

---

## 0. What the machine was doing, because last time nobody said

Every number in the wave-3 version of this document was measured while four other agents were
using the same laptop — cargo builds, extra PocketIC instances, other screenshot runs. None of
that was recorded, so none of those figures could be compared with anything, including with
themselves next wave.

The measurement now **samples the machine while it measures the app** (`tools/shots/lib/load-watch.mjs`),
and the samples are in the JSON next to the timings. Conditions for the run reported in §1:

| | |
| --- | --- |
| machine | Apple M4 Max, 16 cores, 128 GiB, darwin 25.3.0 |
| 1-minute load average during the run | 2.84 – 3.46, i.e. **0.18 – 0.22 per core** (12 samples, 10 s apart) |
| total CPU across all processes | 120 – 395 % of 1600 % available |
| cargo / rustc builds running | **none**, in any sample |
| other browser-driving run | **impossible during the measurement.** The app hardcodes `127.0.0.1:4943`, so the port shim is exclusive: only one screenshot-or-perf run can exist at a time. This one waited for the previous agent's run to release the port and then held it for the whole measurement |
| local replica | 66 – 79 processes of the managed network — that is the system under test, not contention |
| also present | one idle 8-hour-old standalone PocketIC test server (7 processes, ~9 % of one core) left by another agent's test harness |

**Exclusive access to this laptop was not available and is not claimed.** What is claimed is
the specific, recorded state above, and one structural fact: the 4943 shim cannot be shared, so
no other browser was driving this replica while the stopwatch was running.

### What changed on a quiet machine: almost nothing

This is the useful result, and it is not the expected one.

| measure | wave 3 (unstated load) | wave 4 (§0 conditions) | delta |
| --- | ---: | ---: | --- |
| action feedback p50 | 15.1 ms | **15.3 ms** | +0.2 ms |
| action feedback p95 | 16.3 ms | 16.8 ms | +0.5 ms |
| `player_action` returns p50 | 190.2 ms | **207.0 ms** | +16.8 ms |
| `player_action` returns p95 | 473.4 ms | 481.0 ms | +7.6 ms |
| settled visible state p50 (money-moving) | 257.9 ms | **356.7 ms** | +98.8 ms |
| settled visible state p95 (money-moving) | 464.2 ms | 481.0 ms | +16.8 ms |
| lobby FMP p50 | 201.0 ms | **198.2 ms** | −2.8 ms |
| lobby FMP p95 | 250.8 ms | 287.3 ms | +36.5 ms |
| table entry → on-chain state p50 | 303.7 ms | **301.4 ms** | −2.3 ms |
| frame interval p50 / p95 | 8.3 / 9.9 ms | 8.3 / 9.7 ms | — |
| frames over 20 ms | 0 of 1816 | **0 of 1869** | — |

Nothing moved by more than noise, and the two figures that moved most got **worse**, not
better. So the wave-3 numbers were not materially inflated by machine load: what they were
measuring — a local consensus round plus a 500 ms poll — does not care much about a laptop
that is 20 % busy. The 99 ms rise in the settled-state median is a sampling artifact of where
the 500 ms poll happened to land across 20 money-moving actions (§5), not a regression: the
p95 of the same measure moved 17 ms, and `pollWaitAfterCallReturnedMs` is p50 **33.6 ms**,
p95 **684.7 ms** — the same bimodal shape as before.

The honest conclusion: **re-running on a quiet machine did not buy accuracy, it bought a
stated denominator.** The dominant term is the replica, and the way to make these numbers
better is §5.1, not a quieter laptop.

Raw output with every sample and every load reading:
`artifacts/perf/perf-desktop-wave4.json`.

### Mobile, re-run — and NOT on a quiet machine, which the record says out loud

The mobile re-run could not be given the same conditions: it waited on the port shim for
another agent's screenshot run, and a cargo build started during it. Its own record says so,
in the file, in these words:

```
1-minute load stayed at or under 0.28 per core (peak 4.44 across 16 cores);
a cargo/rustc BUILD was running; ANOTHER harness process was on the same replica
for part of this run
```

| measure (390×844) | wave 3 | wave 4, conditions above |
| --- | ---: | ---: |
| lobby FMP p50 | 207.8 ms | 219.1 ms |
| click → on-chain state p50 | 309 ms | 382.5 ms |
| action feedback p50 | 15.0 ms | 15.3 ms |
| `player_action` returns p50 | — | 198.8 ms |
| settled visible state p50 (money-moving) | — | 364.2 ms |

Read the two load-affected rows (FMP, click→state) as **ceilings**, not as the app getting
slower: the run that produced them shared the machine with a compiler. The action-feedback
figure is local UI state and is unaffected, which is itself a small piece of evidence that
the load did not touch the client-side path. Raw output:
`artifacts/perf/perf-mobile-wave4.json`.

---

## 1. The four numbers

Desktop, 1440×900, headless Chromium via Playwright, against the managed local network
(gateway `http://localhost:8077`), under the load conditions stated in §0. Medians and 95th
percentiles are **nearest-rank**, so every value printed is an observation that actually
happened rather than an interpolation.

> The table immediately below is the **wave-3** run, kept because §4's reference comparison and
> §7's gates were derived from it. The wave-4 re-measurement is the middle column of the
> comparison table in §0 and the raw JSON named in §6.

| what | n | p50 | p95 | worst |
| --- | ---: | ---: | ---: | ---: |
| **action feedback** — click → the acknowledgement is on a painted frame | 32 | **15.1 ms** | 16.3 ms | 16.3 ms |
| click → the acknowledgement is in the DOM (pre-paint) | 32 | 6.9 ms | 8.4 ms | 8.7 ms |
| **`player_action` update call returns** | 32 | **190.2 ms** | 473.4 ms | 506.9 ms |
| **action → settled visible state**, money-moving actions only | 20 | **257.9 ms** | 464.2 ms | 473.4 ms |
| action → settled visible state, all actions | 32 | 357.1 ms | 814.8 ms | 931.7 ms |
| **lobby, time to first meaningful paint** (cold navigation) | 20 | **201.0 ms** | 250.8 ms | 359.0 ms |
| lobby first contentful paint | 20 | 152.0 ms | 180.0 ms | 272.0 ms |
| lobby largest contentful paint | 20 | 152.0 ms | 180.0 ms | 332.0 ms |
| **table, row click → on-chain state visible** | 15 | **303.7 ms** | 759.2 ms | 759.2 ms |
| table, row click → felt shell painted | 15 | 33.7 ms | 68.3 ms | 68.3 ms |
| **all-in → showdown, frame interval** | 1816 | **8.3 ms** | 9.9 ms | 10.4 ms |
| the same page, idle — the control | 592 | 8.3 ms | 10.0 ms | 33.3 ms |
| opponent's call → winner banner painted | 5 | **474.8 ms** | 994.4 ms | 994.4 ms |

Frames slower than 20 ms during the all-in → showdown transition: **0 of 1816**. Slower than
34 ms: 0. Slower than 100 ms: 0.

Mobile (390×844) is within noise of desktop on all three load/latency measures: FMP p50
207.8 ms, click→state p50 309 ms, ack p50 15.0 ms.

---

## 2. What each number actually means

**Action feedback = 15 ms.** The click's own `event.timeStamp` to the first
`requestAnimationFrame` callback after `.action-pending` enters the DOM — that is the frame
which presents the spinner. The DOM mutation itself lands at 6.9 ms; the remaining ~8 ms is
one frame of the compositor. This is local UI state (`actionPending = true` at the top of
`handleTableAction`), so it is independent of the canister, and it should be: the client
acknowledges before it knows anything.

**`player_action` returns in 190 ms (p50), 473 ms (p95).** This is the real IC update call —
consensus on a local single-node subnet. It is the floor for anything the chain has to
confirm, and it is the number that would change most on mainnet.

**Settled visible state = 258 ms (p50) for actions that move money.** Measured as: the click,
to the first painted frame on which the money on the felt is *different text* than it was at
click time — the pot, the hero's own stack, or the hero's own live bet. Nothing about spinners
or button states; the actual figures.

Only money-moving actions count toward the headline. A Check moves no chips, so there is
nothing on the felt for the measurement to detect, and including Checks would time whatever
the *opponent* did next. The all-actions row is shown for completeness and is the weaker
number for exactly that reason.

**There is a poll in the middle of that number.** `actionPending` clears in a `finally` the
moment the update call resolves — but nothing re-reads the table there. The new pot arrives on
`+page.svelte`'s 500 ms polling tick. Measured directly, the gap between "the call returned"
and "the felt changed" is p50 **43.7 ms**, p95 **633.4 ms**, worst 725 ms. So half the time the
poll happens to land immediately and costs nothing, and in the tail it costs most of a poll
interval. See §5.

**Lobby FMP = 201 ms** means: `<main data-lobby-state="ready">` *and* at least one real table
row in the DOM, presented on a frame. Not "the spinner stopped" — the lobby's own settled
signal, which is the attribute the screenshot harness also gates on.

**Table entry: 34 ms to the shell, 304 ms to on-chain state.** The felt paints almost
immediately and then waits for `get_table_view()`. An earlier draft of this measurement
reported 32 ms for *both*, because it accepted `.pot-amount` as evidence of chain state — and
the felt renders `.pot-amount` as `--` the instant the shell mounts. The milestone is now a
seated player's chip count, a figure that cannot exist before the canister has answered. The
270 ms difference between the two rows is the size of the error that mistake was hiding.

**Animation: the transition is free.** The frame-interval distribution during the all-in →
showdown transition is *identical* to the same page sitting idle: p50 8.3 ms in both, p95
9.9 ms vs 10.0 ms. Zero frames over 20 ms across 1816 frames. One long-animation-frame entry
per hand, worst 51–70 ms, which is the initial render of the winner banner, not the
transition. The honest statement is not "ClearDeck runs at 120 fps" — it is **"the showdown
transition costs no frames at all in this browser."**

---

## 3. The 120 fps caveat, stated plainly

Headless Chromium's compositor is not locked to a physical display. The shortest frame
interval observed was **6.4 ms**, implying a ~156 Hz tick, and the p50 of 8.3 ms is ~120 Hz.
Those are properties of the harness, not of ClearDeck.

The control that makes this checkable: **PokerNow, measured in the same browser on the same
machine with the same code, also idles at a p50 frame interval of 8.3 ms.** Identical. So
8.3 ms is what this rig reports for any page that is not dropping frames, and the only
defensible reading of the animation numbers is the *jank* count (0 frames over 20 ms) and the
*delta against idle* (zero), not the reciprocal.

To get a real display-refresh number, this measurement has to be re-run headed on known
hardware. It has not been.

---

## 4. The reference clients

The task asked for whatever comparable numbers can honestly be obtained. Here is exactly what
was and was not obtainable.

**How to read the provenance tags**, which are now on every cell rather than implied:

* **MEASURED** — this rig, this browser, the client's own timing APIs. A number.
* **BOUND FROM A RECORDING** — *not* a measurement. Derived from a frame capture whose sample
  interval is far coarser than the thing being timed, so it can only say "no longer than the
  sample interval". Comparing it with a MEASURED median is not a comparison.
* **NOT MEASURED** — no instrument was ever pointed at it. Empty on purpose.

| client | how | FCP | LCP | idle frame interval | click→ack | action→settled | animation cadence |
| --- | --- | ---: | ---: | ---: | --- | --- | --- |
| **ClearDeck** | localhost, real canisters | MEASURED 152 ms (p95 180) | MEASURED 152 ms (p95 180) | MEASURED 8.3 ms (n=592) | MEASURED 15.1 ms | MEASURED 257.9 ms | MEASURED 8.3 ms, 0 janky frames |
| **PokerNow** | public internet, no account | MEASURED 684 ms (p95 1656) | MEASURED 744 ms (p95 2056) | MEASURED 8.3 ms (n=479) | NOT MEASURED | **BOUND FROM A RECORDING: ≤ 0.70 s** — a ceiling read off a capture sampled every 0.70 s, not a timing | NOT MEASURED |
| **PokerStars** | — | NOT MEASURED | NOT MEASURED | NOT MEASURED | NOT MEASURED | NOT MEASURED | NOT MEASURED |
| **GGPoker** | — | NOT MEASURED | NOT MEASURED | NOT MEASURED | NOT MEASURED | NOT MEASURED | NOT MEASURED |
| **WPT Global** | — | NOT MEASURED | NOT MEASURED | NOT MEASURED | NOT MEASURED | NOT MEASURED | NOT MEASURED |

**No cell in this table is an estimate.** Nine cells are measurements, one is an explicit
upper bound whose derivation is below, and the rest are empty. If a later wave adds a figure
inferred from a video, a vendor claim or a review, it belongs in a row tagged **ESTIMATED**
with the source named — never in an untagged cell beside a measurement.

**The load columns are not a race.** ClearDeck's 152 ms is a loopback fetch from a replica on
the same machine. PokerNow's 684 ms crossed the public internet to a CDN. Comparing them as
speeds would be dishonest; they are in the table because the *idle frame interval* column is
the control described in §3, and that column is a fair comparison — same browser, same
machine, same instrument.

**PokerNow, what was measured and what was not.** 10 cold loads of the public landing page
with no account, no game joined, no consent banner clicked and nothing involving money. That
yields load timings and idle cadence. Click-to-acknowledge and animation cadence require a
seat at a table, so they were not measured rather than estimated.

**PokerNow, the one gameplay bound.** `$SCRATCH/reference/pokernow/live/` holds a 200-frame
capture of a real public play-money table, 139.5 s long, sampled every **0.70 s** (min 699 ms,
max 703 ms). At that rate frame timing is unmeasurable, but state transitions can be bounded,
and across three street transitions and one pot award **no intermediate state was ever
captured in any sample**:

| transition | last frame before | first frame after | bound |
| --- | --- | --- | --- |
| bets collected into the pot + flop dealt | `t=+91.86 s` pot label absent, board empty | `t=+92.56 s` pot `300`, 3 board cards | ≤ 0.70 s |
| pot awarded, table cleared for the next hand | `t=+123.41 s` pot `300` | `t=+124.11 s` pot `0` | ≤ 0.70 s |

So PokerNow's bet-to-pot collection and its pot award each complete inside 0.70 s. ClearDeck's
comparable figure — opponent's call to winner banner on screen — is p50 **474.8 ms** in wave 3
and **467.6 ms** in the wave-4 re-run, inside that bound, with a p95 of 990.9 ms that is only
just inside it. That is the most that recording supports; it does not support a frame-rate
comparison and none is offered.

**This is a ceiling, not a time, and the asymmetry runs against us.** PokerNow's true figure
could be 50 ms or 690 ms — the recording cannot tell, and 0.70 s is simply how often the
camera blinked. ClearDeck's 467.6 ms is a median of five measured transitions. A reader who
reads "467.6 ms vs ≤ 0.70 s" as "ClearDeck is faster" has been misled by the layout, which is
why the cell in the table above is tagged **BOUND FROM A RECORDING** and this paragraph exists.
The only way to turn it into a comparison is a capture of PokerNow at a frame rate finer than
the transition being timed, which this environment cannot produce.

**Why the native clients are empty.** The reference corpus
(`$SCRATCH/reference/INDEX.json`, 438 files) contains, for PokerStars, GGPoker and WPT Global,
only **static** images: Apple App Store creatives fetched through the public iTunes lookup API
plus vendor and review-site screenshots. There is no video, no frame sequence and no timestamp
metadata for any of them — `find $SCRATCH/reference -name '*.webm' -o -name '*.mp4' -o -name
'*.gif'` returns nothing. Frame-to-frame timing of a bet-to-pot animation cannot be derived
from a still, so it is not derived. Getting these numbers needs a screen recording of the real
client at a known frame rate, which this environment cannot produce.

---

## 5. What the numbers say to do next

**1. Re-read the table when the action returns, instead of waiting for the poll.**
`handleTableAction` in `src/cleardeck_frontend/src/routes/+page.svelte` sets
`actionPending = false` in its `finally` and returns. The felt then updates on the next 500 ms
tick of `BALANCE_REFRESH_INTERVAL`/`loadTableState`. Measured cost: p50 43.7 ms, **p95
633.4 ms**, worst 725 ms of pure waiting after the canister had already answered. A
`loadTableState()` on the success path would move the settled-state p95 from 464 ms to roughly
the call-return p95 of 473 ms — and, more to the point, would remove the tail where the player
has seen the spinner stop and the money has not moved yet.

**2. The action clock on mobile (corrected by the UI/UX wave).** This item used to say
the clock was invisible under 900 px because `.turn-timer` lived inside the hidden
`.feed-container`. It no longer does: the clock is the conic ring around the acting
avatar plus its digits (`SeatPod.svelte`, the digits on the hero's plate row and, in
portrait, on an opponent's avatar), and the `.actions-clock` line across the top of the
action row (`ActionBar.svelte`), 2 px on desktop and 4 px on a phone. The audit measured
the earlier phone form at 13 px of type over a 3 px line; the mobile phase set the floors
in `tools/shots/lib/touch-targets.mjs` (digits at least 11 px, the line a full space
unit) and `tools/shots/touch-targets.mjs` measures them on every table scene. In the last
ten seconds the primary button carries the seconds too ("Call 0.10 · 8s", painted from
`data-secs` by a pseudo-element so the button's text stays the chain-asserted "Call X");
`tools/shots/probe-time-bank.mjs` photographs it.

**3. Re-run headed before quoting any frame rate.** §3.

---

## 6. Method, so this can be re-run

```bash
# the local stack must already be up; this measures it, it does not start it
./scripts/dev.sh doctor

# ClearDeck: all four measurements, desktop
node tools/shots/perf.mjs --loads 20 --entries 15 --actions 32 --hands 5 \
  --json artifacts/perf/perf-desktop.json

# mobile viewport (load, entry and action only)
node tools/shots/perf.mjs --viewport mobile --only load,entry,action \
  --loads 20 --entries 15 --actions 20 --json artifacts/perf/perf-mobile.json

# the reference client, same instrument, no account and no interaction
node tools/shots/perf-reference.mjs --loads 10 --idle-ms 4000 \
  --json artifacts/perf/perf-reference.json
```

Raw output, including every individual sample, is in `artifacts/perf/`. The wave-4 re-runs are
`perf-desktop-wave4.json` and `perf-mobile-wave4.json`.

**Every run now records the machine.** `machineLoad` in the JSON carries the CPU model and core
count, a load-average sample every 10 s, the count of cargo/rustc, PocketIC, icp-cli and
harness processes at each sample, and a one-line verdict. Nothing has to be remembered or
claimed after the fact:

```bash
node -e 'import("./tools/shots/lib/load-watch.mjs").then(m=>console.log(m.sampleLoad()))'
jq .machineLoad.verdict artifacts/perf/perf-desktop-wave4.json
```

Writing that sampler produced two of its own corrections, both worth knowing before trusting a
`ps`-based load figure on macOS: matching the whole command line counted the
`/bin/zsh -c "node …perf.mjs"` wrapper as a second harness process (so a run reported
contention that did not exist), and the managed network's binary lives under
`…/Library/Application Support/…`, so splitting the `comm` column on whitespace loses it
entirely and reports **zero** replica processes on a machine running 73.

**Time base.** Every timestamp is `performance.now()` taken *inside the page*. Click times are
the browser's own `event.timeStamp` on the real click, not the time the driver issued it.

**"Visible" means presented.** A milestone is recorded at the first `requestAnimationFrame`
callback *after* the DOM change that carries it, because that is the frame which will paint
it. The pre-paint DOM time is recorded separately (`domAcknowledgementMs`) rather than hidden,
since reporting only the DOM time would flatter the app by a frame.

**Nothing is mocked.** The app runs through the same port shim and the same deployed asset
canister as the screenshot harness. Every action is a real `player_action` update call. The
opponent is driven from Node through the same public API — it opens for the minimum whenever a
betting round is fresh, so the hero faces a real amount to call on most decisions and the
settled-state milestone has money to detect. `fixture` in the JSON records how many hands and
opponent actions each run consumed.

**Animation runs with motion ON.** The perf context sets `reducedMotion: 'no-preference'`,
which is the opposite of the screenshot harness — the screenshot harness suppresses motion for
determinism, and doing that here would suppress the very transitions being timed.

**Quantiles are nearest-rank.** With n=5 hands or n=20 actions, an interpolated p95 is a
number nobody observed. Every value in this document happened.

---

## 7. Regression use

These are re-runnable gates, not a one-off report. The re-run commands in §6 write the same
JSON shape every time, so a later wave can diff them. Suggested thresholds, set from the worst
observation here rather than from the median:

| gate | threshold | wave 3 | wave 4 |
| --- | ---: | ---: | ---: |
| action feedback p95 | ≤ 33 ms (two frames) | 16.3 ms | 16.8 ms |
| settled visible state p95, money-moving | ≤ 1000 ms | 464.2 ms | 481.0 ms |
| lobby FMP p95 | ≤ 500 ms | 250.8 ms | 287.3 ms |
| table on-chain state p95 | ≤ 1500 ms | 759.2 ms | 741.7 ms |
| frames over 34 ms during the showdown transition | 0 | 0 | 0 |

None of these is wired into `./scripts/dev.sh` yet. That is one target away and is the obvious
next step for whoever owns the gate script. **A gate on these numbers must read
`machineLoad.verdict` too**, or the first busy-laptop run will trip it and teach everyone to
ignore it.

---

## 8. Freshness: the one number on screen that is never re-read

Everything above measures how fast the screen catches up with the chain. One figure never
catches up at all, and it is the only figure on screen that is not on-chain: the **fiat
conversion in the deposit modal**.

`DepositModal.svelte` calls `loadPrices()` once, from `onMount`, un-awaited. There is no
refresh, no `as of` label, no age, and no expiry. A modal left open renders

```
Your ICP Wallet Balance   0.0006 ICP  (~$0.0013)
```

with the same price an hour later, presented exactly as it was presented one second after the
fetch. The crypto figure beside it is re-read from the ledger; the dollar figure is a snapshot
wearing the same styling. On top of that, `priceError` is assigned when the fetch fails and is
**rendered nowhere** — no `.price-error` element exists in the component — so a failed quote
prints an empty `()` rather than saying the price could not be read.

The screenshot harness cannot fix the client, so it does the next best thing and dates the
number: `checks.chain_deposit.onChain` now carries `icpUsdQuote` (the quote actually served,
with the time it was read), `fiatQuoteAgeMsAtAssertion`, and two flags that are `false` today —
`fiatIsRefreshedWhileOpen` and `fiatShowsItsOwnAge`. It also never replays a cached quote: a
volatile host is fetched live, or fulfilled 503 so the app shows its own no-price state, or
explicitly labelled `FIXTURE` in the manifest and in `INDEX.md`
(`tools/shots/lib/config.mjs`, `lib/browser.mjs`).

**The fix, for whoever owns `DepositModal.svelte`** — three small changes, in order of value:

1. render the age: `(~$0.0013 · price read 14 s ago)`, from the timestamp of the fetch;
2. re-fetch on an interval while the modal is open (60 s is finer than any use of this
   number), and drop the figure entirely once it is older than, say, five minutes;
3. render `priceError` — `{#if priceError}<span class="price-error">price unavailable</span>{/if}` —
   so a failed quote is an honest absence rather than empty parentheses. The harness already
   scrapes `.price-error`; the selector has simply never matched anything.
