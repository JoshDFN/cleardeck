# The Design Bar

What the best real-money poker clients in the world actually look and feel like, measured.

This file exists so that look-and-feel arguments about ClearDeck are settled against
**observed reality** rather than taste. Every number below was measured from a real
client. Where a number is exact it says so; where it is an estimate from image
segmentation it says so and gives the tolerance.

- **Reference corpus** (third-party screenshots, not in this repo, not redistributable):
  `$SCRATCH/reference/` with full provenance in `$SCRATCH/reference/INDEX.json`.
  Every file there maps to its source URL, the client and scene it shows, and whether it
  is **real gameplay** or **marketing**.
- **Collected**: 2026-08-04. Clients: PokerNow, PokerStars, GGPoker, WPT Global.

---

## 0. How the corpus was collected, and what that means for trust

| Client | How we got pixels | Trust level |
|---|---|---|
| **PokerNow** | Captured by us: Playwright driving real Chrome 150, `devicePixelRatio 2`. A **live 10-seat public play-money game** observed as an unseated spectator (200 frames at 700 ms), plus PokerNow's own **production table renderer** driven through its interactive tutorial (80 frames). Plus DOM geometry, computed styles and the shipped CSS bundle. | **Highest.** Exact CSS pixel values and exact animation durations. |
| **PokerStars** | Native client cannot be installed here. Real client screenshots from review sites + official App Store creatives. | Medium. Ratios ±5%; colours reliable. |
| **GGPoker** | Same. | Medium. |
| **WPT Global** | Same. | Medium-high (several full 1124–1440px desktop client window captures). |

### 0.1 Provenance corrections applied 2026-08-04

**Sixteen files were filed under the wrong client, and the index's own `source_page` URLs
prove it.** The App Store harvester walked neighbouring bundle ids and attributed both to
whichever vendor it was collecting:

| Bundle id | Filed as | Actually | Files |
|---|---|---|---|
| `id1529839330` | `ggpoker` | **ClubGG Poker** — GGPoker's separate free/club app, a different product with a different table UI | 10 |
| `id1571146807` | `wptglobal` | **ClubWPT Poker & Casino** — a subscription/sweepstakes product under the WPT brand, **not** WPT Global's real-money client | 6 |

Any per-client aggregate computed before this correction mixed **three** products into two.
`INDEX.json` has been rewritten: those files now carry `client: clubgg` / `client: clubwpt`,
the original wrong label is preserved in `client_original_wrong`, a `corrections` block
records why, and the per-client counts were recomputed (GGPoker marketing 39 → 29, WPT
Global marketing 17 → 11). The pre-correction index is kept at
`INDEX.json.pre-reattribution`. **All four are marketing creatives, so no measurement in
this file was derived from them** — but a future reader browsing "the GGPoker folder" would
have been looking at two different products.

**Gutter contamination.** Vendor-client screenshots taken off review pages carry 125–184 px
of the review site's own white page baked into the right edge. Anything measured relative to
"the image" was therefore measuring the review website as much as the poker client, and the
old "surround share" number for at least one file was literally the review site's white
gutter and its affiliate button. Every measurement in §1 crops the gutter first and is
relative to the client window only.

Two things we deliberately did **not** do, and which cap what this corpus can contain:

1. **We did not create a PokerNow game.** Game creation is behind a Cloudflare Turnstile
   bot check. Defeating or completing a bot check is off-limits, so we used the live
   public games and the tutorial renderer instead. Consequence: no PokerNow 2-seat
   heads-up table, and no seated view (so no PokerNow bet-sizing control strip).
2. **We clicked no consent, ToS or age-gate banner anywhere.** Where such a banner
   covered a reference screenshot we hid it with local CSS for the capture only. Some
   raw frames still show PokerNow's ToS strip along the bottom; that is why.

---

## 1. Cross-client measured bars

The single most useful number is **how much of the window the playing surface occupies**,
because it determines whether a table feels like a game or like a web page with a
picture of a table on it.

> **These numbers were re-derived from scratch on 2026-08-04 and several of them replaced
> the originals.** The first pass measured the wrong thing three times over: it caught
> PokerNow's `.table` DIV (rail plus the whole pod row) instead of the green ellipse; it
> reported "surround share" as `palette[0].pct` of a whole-file colour quantize, which for
> at least one file was measuring the *review website's white gutter and its affiliate
> button*; and it measured window-relative percentages on files that still had 125–184 px
> of that review-page gutter baked into the right edge. The corrected method, the
> verification of it, and the honest gaps are in §1.1. What follows is the corrected set.

### 1.1 How the playing surface is measured now, and how it was checked

1. **Crop the review-page gutter first.** Most vendor-client captures came off review
   pages as element screenshots, so a near-white strip of the review site sits at the
   right edge. Columns that are ≥97% one bright desaturated colour are cropped before
   anything is measured. Removed: **128 px** from `web-ps-gipsy-1.png`, **125 px** from
   `web-gg-gipsy-4.png`, 0 from the rest. Every percentage below is relative to the
   **client window only**.
2. **Hue-mask the felt, then take the largest 4-connected component** and its bounding
   box. Per-file hue bands, because the four clients ship different felts (mid green, dark
   green, dark teal, bright teal) at different JPEG qualities.
3. **Sanity-check the box three ways**, all tolerant of the cards, chips, avatars and
   watermarks that sit *on* the felt and are excluded by the mask (which is why any check
   that walks a single scanline through the middle is worthless — an early attempt at one
   reported PokerNow's minor axis 21% short):
   - `fill_vs_ellipse` = blob area ÷ π·a·b. A clean ellipse with overlays lands 84–96%; a
     rounded rectangle or stadium exceeds 100%; a mask that has leaked into the background
     lands nowhere near either.
   - `widest_row_rel` and `tallest_col_rel`: where in the box the widest row and tallest
     column sit, 0–1. An ellipse or stadium gives ≈0.5. A stray connected sliver dragging
     the box outwards gives ≈0 or ≈1 — that is the exact failure mode behind the original
     wrong numbers, so it is now checked every time.
4. **Replace "surround share" with an explicit mask ratio**: felt-mask pixels vs
   everything else, over the client window only. No quantize, no palette bucket.
5. **Eyeball every detection.** The measurer writes a magenta-box overlay per file and all
   of them were inspected; the four table captures below are the ones whose detection is
   visibly correct.

Reproduce: `S=<scratchpad> python3 $SCRATCH/v-felt.py` (universal band, gutter crop,
axis cross-check) and `$SCRATCH/v-felt2.py` (per-file bands, fill + centroid checks);
overlays land in `$SCRATCH/v-felt-out/`.

### 1.2 The corrected cross-client table

| Bar | PokerNow | PokerStars | GGPoker | WPT Global |
|---|---|---|---|---|
| Felt colour (sampled) | `#26804D` | `#146127` green theme; **Aurora theme has no coloured felt at all** | `#0B4E10` (green) / `#014F59` (teal) | `#00757D` / `#00737A` |
| Surround / out-of-felt colour | `#242324` | `#17191D` | `#132013` / `#0C1719` | `#080925` / `#04031C` |
| **Not-felt share of the client window** (explicit mask ratio) | **74.0%** | **76.6%** | **74.5%** | **71.5–77.9%** |
| **Playing-surface aspect (w:h)** | **2.00** (two viewports, identical) | **≥2.19** green theme (client clipped at right); **1.72** Aurora | **2.17** | **2.08** and **2.62** (two different table arts) |
| **Surface width ÷ client window width** | **62.5%** | **≥81.6%** green; 79.6% Aurora | **75.3%** | **73.1–74.1%** |
| Surface height ÷ window height | 50.1% | 43.3% green; 64.5% Aurora | 49.4% | 40.2–53.1% |
| Shape (from `fill_vs_ellipse`) | ellipse (96%) | ellipse (84%) | ellipse (85%) | ellipse 92% / stadium 105% |
| Board card width ÷ surface width | **13.0%** (exact) | ~8.6% (±) | not reliably measurable | ~6.1% (±) |
| Seat pods sit… | **off the felt**, on the surround | on the rail | on the rail | on the rail |
| Compliance / responsible-gaming text | bottom strip | persistent "Responsible Gaming" **inside the table window**, top-left | footer only | footer only |

**All eight reference measurements, and ClearDeck measured with the identical tool:**

| Capture | Client window | Surface | Aspect | Width % | Fill vs ellipse | Not-felt % |
|---|---|---|---|---|---|---|
| `pokernow/table-9max-desktop-1` | 1512×945 | 945×473 | 2.00 | 62.5% | 96.2% | 74.0% |
| `pokernow/_m-1280x800` | 1280×800 | 800×401 | 2.00 | 62.5% | 96.3% | 73.9% |
| `pokerstars/web-ps-gipsy-1` (green) | 670×577 | 547×250 | ≥2.19 | ≥81.6% | 84.0% | 76.6% |
| `pokerstars/web-ps-wpd2-2` (Aurora) | 732×524 | 583×338 | 1.72 | 79.6% | n/a | n/a |
| `ggpoker/web-gg-wpd2-2` | 900×632 | 678×312 | 2.17 | 75.3% | 84.6% | 74.5% |
| `wptglobal/web-wpt-tphB-2` | 1124×754 | 833×400 | 2.08 | 74.1% | 92.2% | 71.5% |
| `wptglobal/web-wpt-beasts-1` | 1440×1000 | 1053×402 | 2.62 | 73.1% | 95.3% | 77.9% |
| `wptglobal/web-wpt-beasts-3` | 1439×994 | 1053×401 | 2.63 | 73.2% | 104.5% | 75.8% |
| **ClearDeck** `table-preflop-desktop` | **1440×900** | **876×476** | **1.84** | **60.8%** | **104.7%** | **71.1%** |

Reference aspect: range **1.72–2.63**, median **2.13**. Reference width share: range
**62.5–81.6%**, median **73.7%**.

Four things jump out, and they are the real strategic findings:

1. **The field does NOT split on aspect ratio. Every leader is a wide stadium.** Seven of
   the eight reference measurements are **2.00 or wider**; the eighth is 1.72. The
   original claim that "GGPoker is nearly round (1.1–1.3)" was an artefact of measuring
   the wrong rectangle — GGPoker is **2.17**, and its real-gameplay capture shows an
   unmistakable ~2:1 stadium. This is the single most consequential correction in this
   file, because **ClearDeck's table is a rounded rectangle at 1.84** (its 104.7% ellipse
   fill is the straight sides showing up in the arithmetic), so it is both the flattest
   *and* the only non-elliptical surface in the comparison.
2. **Every one of these clients is majority dark surround, and by a consistent margin.**
   Measured properly as a mask ratio, **71.5–77.9%** of the client window is *not* felt.
   The old 52–82% spread was noise from the quantize. ClearDeck's 71.1% already sits at
   the edge of that band, so this is the one bar we broadly clear.
3. **PokerNow's board is roughly twice as large, proportionally, as anyone else's.** A
   single board card is **13.0%** of surface width on PokerNow versus ~6.1–8.6% on WPT
   Global and PokerStars, and the five-card board spans **69.7%** of surface width. On
   ClearDeck the board card is **6.8%** of surface width and the board spans **37.9%** —
   the paid-client end of the range, on a browser-first client that has PokerNow's
   constraints rather than a native client's.
4. **PokerStars' Aurora theme abolishes the felt/surround distinction entirely.** Sampled
   pixels inside and outside the table are the *same* colour (`rgb(23,42,51)`); the
   playing surface is delineated by a thin light outline, not a fill. So "the felt must be
   a minority of dark pixels" is not a universal law of the category — one of the two
   biggest operators ships a table with no felt to measure. Any bar phrased in terms of
   felt colour has at least one serious counter-example.

---

## 2. PokerNow — measured exactly

PokerNow is the reference we can measure to the pixel, so it carries the most quotable
bars. All values are CSS pixels at a **1512 × 945** viewport unless stated.

### 2.1 Layout geometry (exact, from DOM `getBoundingClientRect`)

| Element | Size | Notes |
|---|---|---|
| `.table` (whole playing area incl. surround) | 1511.8 × 714.4, top at y=47.3 | 100% of window width, **75.6% of window height** |
| DOM box previously reported as the felt | 912 × 570 | 60.3% of window width and height, aspect 1.60. **This is not the extent of the painted green surface — see the row below.** |
| **Visible green playing surface** (measured from pixels) | **945 × 473** | **62.5% of window width, 50.1% of window height, aspect 2.00** |
| Board card (`.card`, "big") | **122.5 × 149.7** | aspect **0.818**; 13.4% of felt width |
| Five-card board (`.table-cards`) | 658.6 × 149.7 | **72.2% of felt width** |
| Opponent hole card ("med") | 83.0 × 100.7 | **68% of a board card** — opponents' cards are deliberately smaller |
| Seat pod (`.table-player`) | **272.1 × 96.4** | 29.8% of felt width; **outside** the ellipse |
| Player name / stack rows | 122.5 × 24.5 / 103.4 × 21.8 | name above stack, left-aligned in the pod |
| Pot readout (`.table-pot-size`) | 196.5 × 51.7 | centred, **above** the board, 20.8% of surface width |

**On the 912×570 / 945×473 disagreement, because it matters and should not be papered
over.** The 912×570 figure came from a DOM `getBoundingClientRect`; 945×473 is what the
green pixels actually occupy, measured twice at two different viewports (1512×945 and
1280×800), agreeing to 0.1% on every ratio, with a 96% elliptical fill and its widest row
and tallest column both near the centre of the box. The measured surface is *wider* than
the DOM box it was supposed to sit inside, so the two cannot be describing the same thing —
whichever element was captured, it is not the one painting the felt. **Every ratio in this
file is stated against the visible surface (945), because that is what a player sees and
what a design bar has to be written against.** Consequences: the board card is 13.0% of
surface width, not 13.4%; the five-card board spans 69.7%, not 72.2%; the seat pod is 28.8%,
not 29.8%; the pot readout 20.8%, not 21.5%. The card and board *pixel* sizes are unchanged
and still exact — only the denominator moved.

### 2.2 The scaling law (this is the important architectural finding)

PokerNow does not use width breakpoints. It sizes **everything** in `rem` and scales the
root font-size to the viewport. Measured:

| Viewport | Root font-size | Orientation class |
|---|---|---|
| 1512 × 945 | 27.216 px | `landscape` |
| 1280 × 800 | 23.040 px | `landscape` |
| 844 × 390 | 12.480 px | `landscape` |
| 390 × 844 | 22.464 px | `portrait` |

Which fits, to the pixel:

- **Landscape: `root font-size = min(1.8vw, 3.2vh)`**
  (1512×0.018 = 27.216 ✓; 1280×0.018 = 23.04 ✓; 390×0.032 = 12.48 ✓ — height-bound)
- **Portrait: `root font-size ≈ 5.76vw`** (390×0.0576 = 22.464 ✓)

Their only media queries are on **aspect ratio**, not width:
`@media (min-aspect-ratio: 1/1.8)` and `@media (min-aspect-ratio: 16/9)`. Every rule is
authored twice, prefixed `.landscape` and `.portrait`.

**The bar:** one table layout, two orientation variants, zero breakpoints, and the whole
table scales as a single rigid object. Any client that reflows piecemeal at breakpoints
will feel worse than this at intermediate sizes.

### 2.3 Motion (exact, from the shipped `game-*.css`)

| What | Duration / curve | Selector |
|---|---|---|
| Card flip | **150 ms** | `.card-container .card-flipper-animated { transition: .15s }` |
| Board card reveal (deal to felt) | **500 ms** | `.table-cards .card-container.up` |
| Player hole cards turning up | **500 ms** | `.table-player .card-container.up` |
| Your own cards hiding | **300 ms** | `.you-player .table-player-cards.hide .card` |
| Dealer button travelling to the next seat | **500 ms** | `.dealer-button-ctn` |
| Pot increment ("add-on") | **500 ms** | `.table-pot-size .add-on-container` |
| Current player's turn glow | **400 ms**, fill-mode forwards | `glow-current`: box-shadow `0 0 0 #fff` → `0 0 2.1em #fff` |
| **Winner celebration** | **4 s total**: grow 1 s → decrease 1 s (delay 1 s) → pulse 2 s (delay 2 s) | `glow-grow`/`glow-decrease`/`glow-pulse`, colour **`#E9FF63`**, up to `0 0 3em` |
| Action signal blink | 1 s linear infinite (opacity 0→1→0) | `.action-signal:after` |
| Bomb-pot banner | 800 ms visibility+opacity | `.bomb-pot-banner` |

**The bar:** 500 ms is the house tempo for anything that *moves an object* (card to felt,
button to seat, chips to pot). 150 ms for a card flip. The only long animation is the
**4-second winner glow** — the single moment they let breathe.

### 2.4 Typography and colour (exact)

- **Exactly two typefaces on the entire table.** `Mulish` (all UI: names, stacks, pot,
  log) and `Abril Fatface` (card ranks only, 700 weight, 54.4 px at 1512 wide — i.e.
  **2.0× the base UI size of 27.2 px**). Nothing else.
- Card rank red: **`#DB3131`**. Card rank black: **`#2C2C2C`** (not pure black).
- Card face: `#FFFFFF`, `border-radius: 7%`, `box-shadow: 0 0 13.6px rgba(0,0,0,.25)`.
- Green accent (status pips, CTA): `#49A16E`. Pod background: `rgba(51,51,51,.76)`.
- Winner glow: `#E9FF63`. Current-turn glow: `#FFFFFF`.

### 2.5 What PokerNow puts on screen (observed)

- **Pot** as a dark pill centred above the board, with a smaller superscript `total N`
  beside the main figure — main pot and total are two different type sizes in one pill.
- **Bet discs**: flat yellow-green lozenges with the amount in dark text, parked on the
  felt between the pod and the pot, then collected. Not chip stacks — single discs.
- **Empty seats**: full-size pod outline with the word `SIT` centred. The table's shape
  is legible even when nobody is seated.
- **Away / offline**: pod desaturates to grey with a generic person glyph and an `AWAY`
  or `AWAY (OFFLINE)` label, plus `IN NEXT HAND` as a distinct third state.
- **Timer**: a thin two-colour progress line along the bottom edge of the acting player's
  pod. No numeric countdown, no ring.
- **Dealer button**: small white circle with `D`, animated 500 ms between seats.
- Two watermarks on the felt (`No Limit Texas Hold'em`, `POWERED BY POKER NOW`) at very
  low contrast — the felt is never truly empty.

### 2.6 Hand history — the strongest single feature in the corpus

PokerNow's `LOG / LEDGER` opens a **Session Log** modal that is better than anything else
we found, and it is cheap to match:

- White sheet, near-full-width, over a dimmed table.
- One line per action, newest first, each with a `HH:MM` timestamp in grey.
- **Actions are colour-coded**: folds red, calls blue, blind posts green, checks grey,
  street reveals (`Flop: [3♥, 10♥, 3♦]`) in black with real suit glyphs.
- A bold rule separates hands, with a header naming the hand: `-- starting hand #338
  (id: v8huldcrlr7l) No Limit Texas Hold'em (dealer: imaohw) --`. **Every hand has a
  stable public id.**
- A `Player stacks:` line snapshots all stacks at hand start: `#2 Youknowit (10000) | #5
  Roll jacket (27013) | …`
- Footer actions: `HAND #337 »` (per-hand pagination), `DOWNLOAD`, `FULL LOG`, `LEDGER`,
  `REPLAYER`.

**The bar:** plain-language, colour-coded, timestamped, per-hand paginated, addressable
by hand id, and downloadable. For a provably-fair on-chain game this is the natural home
for the fairness proof, and PokerNow has already shown players will read it.

---

## 3. PokerStars

Sources: real desktop client and lobby captures (GipsyTeam, WorldPokerDeals), official
App Store creatives (Stars Mobile Limited, GB + US storefronts).

**Table.** Green `#146127` felt as a rounded stadium on near-black `#17191D`. Measured
palette: 55.7% surround, 22.2% white, 18.0% felt. Board card ≈7.0% of felt width — half
PokerNow's proportion. Seat pods are compact dark plates **on the rail**: avatar tile at
one end, name over stack in two rows.

**Two table skins co-exist.** The classic green felt, and the **"Aurora"** theme which is
radically minimal: a thin light outline ellipse on a near-white ground, pods as outlined
lozenges, and orange/red card faces. Same client, opposite visual language — evidence
that skinning is a first-class feature, not a coat of paint.

**All-in.** Observed on the Aurora table: `All-In` as a text label under the player name,
and **equity percentages rendered as small badges next to each player** (`0%` / `100%`).
Pot as plain text `Pot: $123.15` centred above the board. There is no explosion, no
shake, no sound cue implied — the drama is *information*, delivered as live equity.

**Bet controls (from official creative).** A three-button row across the bottom —
`Fold` | `Check` | `Bet 20,000` — above a horizontal slider flanked by `–` and `+`
steppers, with the current amount shown numerically to the left (`30K`). Buttons are
full-width thirds. No pot-fraction preset chips visible in the captures we have.

**Persistent furniture.** `Responsible Gaming` sits in the top-left of the table window
at all times. Bottom-left of the table is a **tab strip**: `Chat | Hands | Notes | Stats
| Info`, with the hand history rendered inline as plain dealer narration
("Dealer: fso99 has two pair, Kings and Fours"). Hand history is a *tab on the table*,
not a modal.

**Lobby.** A dense data grid: tab row (`Cash | Zoom | 6+ Hold'em | Tempest | Sit & Go |
Spin & Go | Grand Tour | Tourney | SCOOP | Events`) over a sortable table of
Table / Stakes / Game / Type / Plyrs / Wait / Avg Pot / Plrs/Flop / Hs/hr, with
`Play Now` and **`Observe`** as the two row actions. Hundreds of rows visible at once.
This is a professional's instrument, not a storefront.

**Cashier.** A method matrix listing Apple Pay, Rapid Transfer, AstroPay, Skrill,
NETELLER, Visa, Mastercard, paysafecard, direct bank transfer and wire, each with an
explicit deposit ✓ / withdraw ✗ mark. The honesty of showing which rails are one-way is
worth copying.

---

## 4. GGPoker

Sources: real desktop client captures (GipsyTeam, WorldPokerDeals, SoMuchPoker), official
GGPoker feature pages, official App Store creatives (NSUS Limited, GB/CA/US storefronts).

**GGPoker's defining decision: one table engine, radically different dress per game
type.** Across the corpus we see four distinct treatments in the same client:

| Game type | Felt | Table shape |
|---|---|---|
| Cash / NLH Diamond | deep green `#0B4E10` | oval |
| Cash (alt theme) | teal `#014F59` | rounded stadium |
| **Rush & Cash** | amber / orange | oval |
| **All-In or Fold** | dark maroon | **octagonal** |

The octagonal All-In-or-Fold table is the strongest example: the *shape of the table*
changes to signal a different game. Nobody else in the corpus does this.

**Table.** Surround dominates hard — 66.4% (green) to **82.5%** (teal) of the window is
near-black. Seat pods carry **large illustrated avatar portraits** (a 75+ portrait set
ships with the client) rather than photos or initials, with a small numeric badge
attached to each pod. A `DAILY $20,000 LEADERBOARD` strip pins to the top of the table.

**All-in / showdown.** Richest of the four clients:
- `All-In` as a coloured tag under the player's name.
- **Live equity** to two decimals (`100.00%`) in a badge beside the player.
- A **named hand-strength readout on the felt** — e.g. `Sharp red hands` — i.e. the client
  editorialises about the hand in words, not just card ranks.
- `Total Pot : $3,306` as centred text above the board.
- Emoji reaction row and animated "taunts" attached to pods.
- At tournament win: a full-screen overlay (`YOU WON THE TOURNAMENT`, prize, finish
  position) with `Best Hand` / `Bad Beat` / `Last Hand` tabs — the session is summarised
  as three named highlights.

**Smart HUD.** A per-player overlay pinned to the pod showing a large percentage in a
ring (`98%`), name, stack and pot. Stats live in the client, free, for everyone — the
opposite of the third-party-HUD arms race.

**Action bar.** On the All-In-or-Fold table it collapses to exactly **two large buttons,
`Fold` and `All-In`**, occupying the full width. On Rush & Cash a single `Raise 5.40`
button dominates. GG sizes the action bar to the decision, rather than always showing
three buttons.

**Rush & Cash detail worth stealing.** A vertical stack of the hands you just folded is
rendered at the table edge — the fast-fold game shows you your own folding history so the
speed doesn't erase the sense of a session.

**Hand history.** **PokerCraft**, a separate analysis surface with `Game History`, a
`Win/Loss` equity graph over time, a **hole-card matrix** (13×13 grid heat-mapped by
result) and a `Position` breakdown. This is the ceiling for post-session review.

**Jackpot furniture.** A live jackpot counter (`$2,685,149.13`) sits in the table's top
bar, and All-In-or-Fold adds a `Win Up to 80,000 / 43 hands left / Win with 1 Line!`
strip. Persistent, always-visible upside.

---

## 5. WPT Global

Sources: real desktop client window captures at 1124–1440 px (ThePokerHolics,
BeastsOfPoker, SoMuchPoker, WorldPokerDeals), official App Store creatives (Kashxa
Limited, CA storefront).

**Table.** The widest felt in the corpus: aspect **1.80–2.27**, teal `#00757D` on deep
navy `#080925` (57.4% of the window). The felt reads as a long stadium with a visible
raised rail. Accent palette from the same measurement: `#131D55` (navy panels),
`#04878F` (felt highlight), `#EEB441` (gold), `#44AE69` (green CTA). Board card ≈6.5% of
felt width.

**Multiple named table themes ship together.** One review capture shows four side by
side: `Highstakes`, `Jackpot`, `PLO`, `NL Game` — plus a cloud/rose theme in another
capture. Like GGPoker, theme is tied to game context.

**Empty-seat treatment is the best in the corpus.** Empty seats are explicit
**`Sit Here`** labels or large circular **`+`** buttons placed exactly where the pod
would be, with the instruction `Click on an empty seat to sit at the table` in the lobby.
Sitting down is a one-click affordance on the table itself, not a lobby transaction.

**Pot.** `Total pot: ¥34.3` / `Total pot : $123` as centred text directly above the
board, with a separate **`JACKPOT ¥1,208`** readout pinned to the top-right corner.

**Seat pods.** Slim horizontal pills: circular avatar at the leading edge, name above
stack. Currency is rendered explicitly (`$`, `¥`, `CNV`) — a multi-currency client that
never hides which unit you are looking at.

**On-table utilities.** `HANDS` and `GAME LOG` as two small persistent buttons at the
bottom-left of the table. Hand history is one click away, always, from inside the game.

**Lobby.** A left **icon rail** — `Lobby / Cashier / Profile / Invite / Promo / Help` —
with a tabbed content pane (`MTT | NLHE | PLO | Short Deck | Global Spins`) listing
stakes, buy-in and seated count with a `JOIN` button per row. The desktop lobby is a
single window that contains the cashier; you never leave the client to fund.

**Mobile.** The real mobile client keeps the same tab set (`MTT / NLHE / PLO / SHORT DECK
/ GLOBAL SPINS`) as a horizontal scroller and lists stakes as full-width rows with
buy-in and a seated-count pill. Balance and a `+` (deposit) button sit in the header.

**Cashier.** Deposit milestones are published as an explicit table (deposit number →
minimum amount → additional reward), min deposit $10. Rewards are stated as rules, not
vibes.

---

## 6. The bars ClearDeck has to clear

Concrete, checkable targets derived from the above. These are the numbers a look-and-feel
critic should hold ClearDeck to.

> **Bars 1, 2, 7, 10 and 15 were rewritten on 2026-08-04.** As originally written they
> rejected reality: bar 1's window-share range excluded 3 of the 4 reference clients, bar
> 2's aspect-ratio range excluded **all four**, bar 7 quoted a card ratio computed against
> a felt width that was itself wrong, bar 10 promoted a single-client observation to a
> cross-client law, and bar 15 claimed a majority of leaders do something only two of them
> were ever observed doing. Worse, the old bars 1 and 2 were both satisfied by ClearDeck's
> own table (60.8% and 1.84) while excluding the clients they claimed to be derived from —
> a bar that passes the thing it is meant to judge and fails its own references is not a
> bar. Corrected values below, with the measurement in §1.

**Table and layout**
1. Playing surface occupies **70% ±8 of client window width** at desktop (measured range
   **62.5–81.6%**, median 73.7%), with the rest as a dark surround. Do not fill the
   viewport with felt — but do not undershoot either: ClearDeck's **60.8%** is *below every
   reference measurement except PokerNow's 62.5%*, so the table currently reads smaller
   than the category, not more restrained than it.
2. Playing-surface aspect ratio **1.9–2.3** (measured range **1.72–2.63**, median 2.13,
   with 7 of 8 measurements at ≥2.00). **The field does not split here — every leader is a
   wide stadium.** ClearDeck's 1.84 is below the whole reference band bar one, and its
   surface is a rounded rectangle rather than an ellipse or stadium (ellipse fill 104.7%
   against the references' 84–96%). Reshaping the surface to a ~2:1 stadium is the
   single highest-leverage geometry change available.
3. Surround must be genuinely dark (`#17191D`–`#242324` band) and must be the **majority
   of the pixels**: measured **71.5–77.9%** of the client window is not-felt, as an
   explicit felt-mask ratio. ClearDeck is at 71.1%, just inside. Note the counter-example
   before treating this as a law: PokerStars' Aurora theme has **no felt fill at all**, so
   this bar governs *contrast and framing*, not literally "green must be scarce".
4. Empty seats render as a **full-size pod with an explicit call to action**
   (`SIT` / `Sit Here` / `+`). Never a gap.
5. Seat pod width **25–30% of felt width**. Name above stack, two rows, left-aligned.

**Cards**
6. Board card aspect **0.80–0.82**. `border-radius` ~7% of width.
7. Board card width **≥9% of the playing surface width**, target **13%**. Measured against
   the *corrected* surface widths: PokerNow **13.0%** (exact, DOM card 122.5 px ÷ surface
   945 px), PokerStars ~**8.6%**, WPT Global ~**6.1%**, GGPoker not reliably measurable.
   The five-card board should span **50–70%** of surface width (PokerNow: **69.7%**).
   ClearDeck today: board card **6.8%** of surface width, board span **37.9%** — the
   native-client end of the range, on a browser-first client with PokerNow's constraints.
   PokerNow's ratio is the right target precisely because it is the only other
   browser-first client in the corpus, and it is why PokerNow reads well on a phone.
8. Opponents' hole cards render **smaller than board cards** (PokerNow: 68%).
9. Card rank in a display face at **~2× base UI size**, red `#DB3131`-ish, black
   `#2C2C2C` (never `#000`).
10. **Two typefaces total** on the table: one UI sans, one display face for ranks. Status:
    this is a **one-client observation promoted to a house rule, not a measured
    cross-client finding**, and it should be quoted as such. It is exact for PokerNow only
    (`Mulish` + `Abril Fatface`, read from computed styles on a live DOM). For PokerStars,
    GGPoker and WPT Global we have JPEGs and PNGs, and **you cannot read a font stack off a
    raster** — nothing in the corpus establishes their typeface count either way. It is
    still a good rule (two faces is the discipline PokerNow's table legibility rests on)
    and worth adopting; it is not evidence about the category. Where the corpus *does*
    contradict a monoculture: GGPoker's card faces use a visibly different system, a
    four-colour deck with per-suit card backgrounds rather than a single white face.

**Motion**
11. **500 ms** for anything that moves an object: card to felt, dealer button to seat,
    bet to pot, pot increment. **150 ms** for a card flip. **300 ms** for hiding your own
    cards.
12. Acting-player indicator appears in **≤400 ms**.
13. Winner celebration is the one long beat: **~4 s**, in three phases
    (grow → settle → pulse), in a single bright accent (`#E9FF63` is the reference).
14. Nothing else on the table animates longer than 1 s.

**The all-in moment**
15. The reference clients dramatise all-in with **information, not fireworks**: an
    `All-In` tag on the pod, **live equity percentages per player**, and the pot total
    called out. GGPoker adds a named hand-strength phrase.

    **Corrected count: two of four, not three of four.** Equity badges appear in real
    gameplay for exactly two clients, and the evidence is four files:
    - **PokerStars** — `web-ps-wpd-2.png`, `web-ps-wpd2-2.png`: Aurora-theme table at
      all-in with `0%` / `100%` badges beside the two players, `All-In` under the name,
      `Pot: $123.15` above the board. (`web-ps-wpd-2.png` also carries an "Are you over
      18?" age gate that dims the page — usable as equity evidence, useless for geometry.)
    - **GGPoker** — `web-gg-wpd-2.png`, `web-gg-wpd2-2.png`: `100.00%` to two decimals,
      `All-In` tag, `Total Pot : $3,306`, plus the hand-strength phrase "Sharp red hands".

    **PokerNow: no equity anywhere** in 220 real-gameplay frames — it was observed as a
    spectator, and nothing in §2.5 shows an equity display. **WPT Global: none** in its
    real-gameplay captures; its only all-in reference is an illustrated marketing render,
    which §8 already says not to treat as authoritative.

    The recommendation survives the correction and is arguably strengthened: both clients
    that do this are the big-money native clients, and it costs no animation budget.
    ClearDeck should show live equity at all-in. Just do not defend it with "three of four
    leaders do it" — the number is two, and both are natives.

**Showdown**
16. All live hole cards face-up, winning hand named in words, winner pod glowing for
    ~4 s, pot visibly travelling to the winner, then a hand summary.

**Hand history — where ClearDeck should beat everyone**
17. Match PokerNow's Session Log floor: per-action lines, `HH:MM` timestamps,
    **colour-coded action types** (fold red / call blue / blind green / check grey),
    real suit glyphs, a bold per-hand separator carrying a **stable hand id**, a
    `Player stacks:` snapshot, per-hand pagination, and **download**.
18. Then exceed it: the shuffle commitment/reveal and the deal proof belong in this
    panel, per hand id. Every competitor's history is a *narrative*; ClearDeck's can be a
    *proof*. That is the one place we can be categorically better rather than merely
    equal.

**Responsive**
19. Scale the whole table as one rigid object from a single root size, not per-element
    breakpoints. Reference law: **`font-size: min(1.8vw, 3.2vh)`** landscape,
    **`~5.76vw`** portrait, switched on **aspect ratio**, not width.
20. Portrait is a genuinely different layout, not a squeeze: the ellipse becomes a tall
    rounded rectangle and pods move to the left and right edges in two columns, pot
    centred. Table still occupies ~75% of viewport height.

**Cashier**
21. State the rails honestly, including which are deposit-only (PokerStars publishes a
    per-method deposit/withdraw matrix). For ClearDeck: show the ledger, the fee, the
    confirmation depth, and the canister that holds the funds.

---

## 7. Compliance furniture: do not follow the reference here

None of these four clients carries an unaudited-alpha warning, because none of them is
unaudited alpha. So there is **no reference to copy** for ClearDeck's
unaudited-alpha disclaimer, 18+ notice, jurisdiction/legality warning, or the no-rake
property.

What the corpus does show is where compliance text *lives* when a client takes it
seriously: PokerStars keeps **`Responsible Gaming` pinned inside the table window at all
times**, top-left, on every table, in every theme. It is not in a footer and it does not
scroll away.

The correct conclusion for ClearDeck is therefore: **design a permanent home for the
disclaimer, 18+ notice, jurisdiction warning and no-rake statement inside the layout,
and reserve the space for them up front.** They must never be shortened, softened, or
moved somewhere less visible in the name of matching a competitor's cleaner table —
those four clients simply have less to disclose. Making them *more* prominent is always
acceptable; shrinking them to buy felt is not.

---

## 8. Where this corpus falls short

Honest gaps, so nobody over-claims from it.

| Gap | Why | Impact |
|---|---|---|
| **No PokerNow heads-up (2-seat) table** | All 18 public PokerNow games are 10-seat; a 2-seat table needs game creation, which is Turnstile-gated. | Heads-up proportions for PokerNow are unknown. GGPoker heads-up is covered (marketing creative only). |
| **ZERO real-gameplay captures of the bet-sizing action bar, for ANY client** | Verified 2026-08-04 across all 438 files. PokerNow was observed as a spectator (a spectator sees a `JOIN` button where the controls would be), and its 80 production-renderer tutorial frames are a *scripted replay* — the wide control strip at the bottom of those frames is the tutorial's own step navigation (`STEP 1 OF 2`, prev/play/next), not poker controls. The three native clients cannot be installed here, and their real-gameplay captures are all spectator/observer views or all-in moments. What exists for bet controls is **marketing creatives only**. | **This is the single largest hole in the corpus, and it lands squarely on the control surface a poker player touches most.** Nothing in this file constrains slider geometry, pot-fraction button sets, min-raise affordances, or bet-input behaviour. Treat every ClearDeck bet-control decision as *unbenchmarked*. Two routes close it: (a) sit down with real money in one live PokerNow game and capture one's own seated controls, or (b) create a PokerNow game, which is Turnstile-gated and therefore off-limits under the rules we are working to. Neither was done. |
| **No real-gameplay all-in for WPT Global** | Its only all-in reference is an illustrated marketing render. | Do not treat WPT's all-in composition as authoritative. |
| **Board-card ratio not measurable for GGPoker** | GGPoker's cards are a four-colour deck with per-suit coloured card *backgrounds* (blue / black / green / red), so white-rectangle segmentation has nothing to find. | GG card proportions are qualitative only. Note this is a design finding in its own right, not just a measurement failure. |
| **PokerStars' Aurora theme cannot be measured by any felt-colour method** | It has no felt fill: sampled pixels inside and outside the table are the same `rgb(23,42,51)`, and the surface is delineated by a thin light outline. Its geometry here (583×338, aspect 1.72) came from scanning that outline's extent through the surface's centre row and centre column, not from a mask. | Its aspect is trustworthy; its "not-felt share" is undefined and is deliberately left blank in §1.2 rather than filled with a number. |
| **PokerStars' green-theme aspect is a lower bound only** | `web-ps-gipsy-1.png` is cropped at the right edge: the client window itself is cut off, so the felt's measured width (547 px) is truncated while its height (250 px) is intact. | Aspect ≥2.19 and width share ≥81.6%; the true values are higher and unknown. Do not quote 2.19 as *the* PokerStars number. |
| **One PokerStars all-in capture carries an age gate** | `web-ps-wpd-2.png` has an "Are you over 18?" modal dimming the whole page. | Usable as evidence for equity badges; useless for geometry — a naive mask measures the dimming overlay (91.8% of the window) instead of the table. |
| **No screen recordings for the three native clients** | Frame-accurate motion timing needs video; we have exact CSS timings only for PokerNow. | All motion numbers in §6 derive from PokerNow. Treat them as a floor validated on one client, not four. |
| **Playing-surface boxes for the native clients are ±5%** | Colour segmentation on third-party screenshots that include window chrome. Now gutter-cropped and sanity-checked three ways (§1.1), which removes the gross errors but not the residual few per cent. | Only PokerNow's geometry is exact — and it is exact twice, at two viewports, agreeing to 0.1%. |
| **Every reference aspect ratio rests on one capture per table art** | Only PokerNow has two independent captures of the same table. | The **direction** of the §1.2 finding is robust (7 of 8 measurements ≥2.00, from four different vendors, four different measuring sessions). Any single number in that table could move a few per cent. |
| **Deposit/cashier flows are thin everywhere** | Cashiers sit behind login. | We have PokerStars' method matrix and WPT's milestone table, but no real cashier screen. |

### Reproducing the measurements

The corpus is self-contained: it ships the raw measurement outputs and the scripts that
produced them, so every number above can be re-derived or challenged.

```
$SCRATCH/reference/
  INDEX.json                              # 438 files, every one mapped to its source URL,
                                          # client, scene, and real-gameplay vs marketing
  measure2-all-clients.json               # felt bbox + palette + card-width segmentation
  tools/measure2.py                       # the segmentation measurer (pure PIL, no numpy)
  tools/measure_img.py  tools/montage.py   # first-pass measurer, contact-sheet builder
  tools/harvest_appstore.py               # official App Store creative harvester
  tools/build_index.py                    # rebuilds INDEX.json from the provenance files
  pokernow/measure-live.json              # PokerNow exact DOM geometry + computed styles
  pokernow/measure-responsive.json        # the four viewports behind the rem scaling law
  pokernow/pokernow-game-css-snapshot.css # source of every exact animation duration in §2.3
```

The **corrected** measurers, which supersede `tools/measure2.py` for anything geometric,
live in the scratchpad alongside their overlays:

```
$SCRATCH/
  v-felt.py                  universal hue band + review-page gutter crop + axis cross-check
  v-felt2.py                 per-file hue bands + fill-vs-ellipse + centroid sanity checks
  critic-felt2.py            the pass that first caught the wrong-rectangle error
  v-felt-out/
    v-felt-results.json      universal-band results, all clients
    v-felt2b-results.json    per-file-band results
    w-<client>_<file>.png    magenta box = detected surface, yellow = widest row,
                             cyan = tallest column. EYEBALL THESE before quoting a number.
    cd-preflop-desktop.png   ClearDeck's own table measured with the identical tool
    aurora-*.png             the outline-scan overlays for PokerStars' Aurora theme
  reference/INDEX.json.pre-reattribution   the index before the 16-file client correction
```

```sh
# Corrected geometry for every client, plus ClearDeck, with overlays to eyeball.
S=$SCRATCH python3 $SCRATCH/v-felt.py     # universal band + gutter crop + axis check
S=$SCRATCH python3 $SCRATCH/v-felt2.py    # per-file bands + fill/centroid checks

# SUPERSEDED for geometry, kept because its palette sampling is still useful.
# Its felt boxes are the ones that were wrong: it caught PokerNow's `.table` DIV
# (1349x862, aspect 1.56) rather than the green ellipse (945x473, aspect 2.00),
# and its "surround share" is palette[0].pct of a whole-file quantize.
# Re-derive the felt/palette/card numbers for any client, with visual verification.
# Writes measure2.json plus dbg2-<tag>.png overlays so the detection can be eyeballed
# (magenta = detected felt box, cyan = client window, yellow = detected board cards).
python3 $SCRATCH/reference/tools/measure2.py /tmp/dbg \
  pokerstars=$SCRATCH/reference/pokerstars/web-ps-gipsy-1.png \
  wpt=$SCRATCH/reference/wptglobal/web-wpt-tphB-2.png

# Re-derive PokerNow's exact animation timings from the shipped bundle.
grep -oE '(transition-duration|animation)[^;]*' \
  $SCRATCH/reference/pokernow/pokernow-game-css-snapshot.css | sort -u
```

**Caveat to respect when quoting this file:** PokerNow's numbers are exact (DOM + shipped
CSS). PokerStars, GGPoker and WPT Global geometry comes from colour segmentation of
third-party screenshots and is ±5%; their colours are reliable, their motion timings are
unknown. Do not present a ±5% estimate as a hard bar.

---

## 9. Wave-3 re-measurement, and the two bars this file never had

Everything in this section was measured on 2026-08-05 by the wave-3 coherence pass, with the
**same** tool used on the reference corpus (`$SCRATCH/w3-felt.py`: one universal hue rule, largest
4-connected component, gutter crop, `fill_vs_ellipse` and widest-row/tallest-column cross-checks —
the §1.1 method), pointed at the shipped `artifacts/screens/latest/` PNGs and the corpus in one run.
No per-file tuning, no separate tool for us.

### 9.1 ClearDeck's desktop table now clears bars 1 and 2

Wave 2 measured ClearDeck at **876×476, aspect 1.84, `fill_vs_ellipse` 104.7%** — the flattest
surface in the comparison and the only non-elliptical one. The table redesign moved it:

| capture (1440×900) | surface | aspect | width % | fill vs ellipse |
|---|---|---|---|---|
| `table-preflop-desktop` | 962×452 | **2.13** | 66.8% | 88.3% |
| `table-facing-bet-desktop` | 962×452 | **2.13** | 66.8% | 88.3% |
| `table-allin-desktop` | 980×463 | **2.12** | 68.1% | 84.4% |
| `table-empty-desktop` | 984×505 | 1.95 | 68.3% | 91.6% |

**Bar 2 (aspect 1.9–2.3): CLEARED**, and the fill figure says it is a genuine ellipse now rather
than a rounded rectangle wearing one (>100% is straight sides showing up in the arithmetic).
**Bar 1 (70% ±8 of window width): CLEARED at the bottom of the band**, 66.8–68.3%.

Two scenes read 1.93–1.95 with `fill_vs_ellipse` near 61–63% (`table-showdown`, `table-sidepots`).
That is mask contamination from the winner glow and the mint pod ring, not a different table — the
DOM element is the same ellipse in every scene. Reported rather than quietly dropped.

### 9.2 A bar this file was missing: **surface AREA, not just width**

Width share alone is misleading, because it does not know how wide the window is. A player
perceives area. Same measurements, expressed as felt bbox ÷ client window:

| client | area % |
|---|---|
| WPT Global `beasts-1` | 29.5% |
| WPT Global `beasts-3` | 29.7% |
| PokerNow | 31.7% |
| **ClearDeck (preflop)** | **33.6%** |
| **ClearDeck (allin)** | **35.0%** |
| PokerStars (green) | 37.8% |
| **ClearDeck (empty)** | **38.3%** |
| WPT Global `tphB-2` | 40.9% |
| GGPoker | 45.6% |

**Reference median 34.75%.** ClearDeck sits at 33.6–38.3%, i.e. **at or above the reference
median on desktop surface area**.

> **BAR 21 (new).** Desktop playing surface **≥ 30% of the client window by area**. Quote area
> alongside width whenever either is used as an argument; a width-share deficit on a wider window
> is not a deficit.

This matters because it **contradicts the conclusion** wave 3's own table redesign nominated as its
biggest gap ("the felt is 66.8–68.3% of window width against a reference median of 73.5%"). The
width figure is right; the conclusion does not survive an area comparison, and area is what is seen.

### 9.3 The bar this file was missing most: **mobile**

> **Read §9.5 before quoting any "today" figure from this section.** The wave-4 coherence pass
> re-measured all three bars below on the shipping build. BAR 22's figure is superseded (the
> 42.9–52.1% reported in wave 4 was measured with the protected notices hidden; shipping is
> **16.4–18.3%**), BAR 23 is now clear, and BAR 24's *reference* figure does not reproduce.

Sections 1–8 contain no numeric mobile bar at all. That is the single biggest omission in this
document, because mobile is where ClearDeck loses by the largest margin in the whole corpus.

Measured at the **identical 390×844**, ours against PokerNow's real portrait capture (the only
phone reference that is a real capture rather than an App Store creative, and therefore the only
one that can carry a hard bar):

| capture (390×844) | felt | width % | height % | **area %** |
|---|---|---|---|---|
| **PokerNow `mobile-portrait-1`** | 313×548 | 80.3% | 64.9% | **52.1%** |
| ClearDeck `table-showdown-mobile` | 217×319 | 55.6% | 37.8% | **21.1%** |
| ClearDeck `table-empty-mobile` | 217×316 | 55.6% | 37.5% | **20.9%** |
| ClearDeck `table-allin-mobile` | 217×299 | 55.6% | 35.5% | **19.7%** |

Weaker corroboration from the phone marketing creatives, which point the same way: GGPoker 50.7%,
PokerStars 42.7%, WPT Global 35.6%.

> **BAR 22 (new).** Portrait playing surface **≥ 40% of the screen by area** at 390×844.
> Today: **19.7–21.1%**, i.e. **38–40% of PokerNow's** on the same device. Marketing creatives are
> evidence of intent, not of geometry; the hard number is PokerNow's 52.1%.

> **BAR 23 (new).** The felt must be **wholly inside the viewport** in every canonical capture.
> Today two of six mobile table PNGs fail it: `table-preflop-mobile.png` and
> `table-facing-bet-mobile.png` have the pot, the whole board and four of six pods above the top of
> the frame. The measurer detects this without a human: `tallest_col_rel = 0.000` means the mask is
> flush against the top edge of its own bounding box, which a centred ellipse can never be. **Wire
> that check into the screenshot gate** — the DOM assertions cannot see it, and both files were
> filed VERIFIED.

> **BAR 24 (new).** Money glyphs **≥ 12 px tall** in portrait. Today ClearDeck's mobile stack digits
> measure 8–10 px against PokerNow portrait's 15 px. Text follows surface: shrinking the felt
> shrank the numbers a player has to read while deciding.

### 9.4 A bar for the lobby, which this file also never had

Section 3 and section 5 discuss the lobby only in prose, so no reviewer could adjudicate the scene.
This section replaces the prose with numbers, and every threshold below names the capture it was
derived from.

**Definitions, so two people measure the same thing.**

* **First row at** — the y of the *top border* of the first table row (desktop grid) or first table
  card (phone), over the viewport height. ClearDeck: `getBoundingClientRect().top` of
  `tbody tr:first-child`, read in the real browser the screenshot harness drives. Reference
  clients: pixel inspection of the capture, relative to the **client window**, gutter cropped
  (§0.1).
* **Rows fully visible** — rows whose top *and* bottom are inside the viewport with no scrolling.
  A row you can see the first line of is not a row you can compare.
* **Data fields per row** — distinct facts the row states about the table, not counting the action
  button.

#### 9.4.1 What the reference clients do

| client | capture | window | first row at | rows fully visible | data fields per row | master/detail preview |
|---|---|---|---|---|---|---|
| WPT Global | `wptglobal/web-wpt-tphB-1.png` | 1127×798 | y≈212, **26.6%** | 5 (every table it had) | **4** — name, table id, stake, players | yes: stadium, `+` on every empty seat, "Click on an empty seat to sit at the table" |
| PokerStars | `pokerstars/web-ps-wpd2-1.png` | 732×552 | y≈190, **34.4%** | **24** | **9** — Table, Stakes, Game, Type, Plrs, Wait, Avg Pot, Plrs/Flop, H/hr | yes: seated-player list, `Play Now`, `Observe` |
| PokerNow | `pokernow/lobby-community-1.png` | 1512×945 | y≈341, **36.1%** | 7 (8th clipped) | **4** — Game, Blinds, Buy-in (Min/Max), Players | no |

Reference band **26.6% – 36.1%**, median 34.4%. Two of the three ship a preview pane; the widest
data grid is 9 columns and the narrowest is 4. All three put filters/tabs above the list and
**clip none of them**: WPT Global's five stake tabs (`MICRO LOW MID HIGH OPEN SEATS`) and its three
game tabs are each fully rendered at 1127 px.

~~**There is no real mobile lobby capture in the corpus**~~ — **THIS WAS FALSE. Corrected by the
wave-4 coherence pass, 2026-08-05; see [DEFECTS.md D-04](DEFECTS.md#d-04).** The paragraph that
stood here searched `INDEX.json` for `scene == 'lobby-mobile'`, found only two WPT Global App Store
creatives, and concluded that no mobile lobby bar could be set — for exactly the metric ClearDeck
fails worst.

`pokerstars/web-ps-gipsy-2.png` is a real one. It is indexed `client=pokerstars`,
`real_gameplay=true`, `capture_type: "third-party review (real client screenshots)"`, notes
"Real PokerStars mobile client: lobby list, Spin&Go table, store", and it is filed
`scene == 'mobile-portrait'`, which is why the query missed it. Its left panel is a complete phone
screen — app top bar, tournament lobby list, bottom tab bar — and it measures cleanly with a row
luminance-step detector over the panel:

| client | capture | screen | first row at | rows fully visible | row pitch |
|---|---|---|---|---|---|
| **PokerStars mobile** | `pokerstars/web-ps-gipsy-2.png` (left panel) | rows 0..567, **568 px** | y ≈ 116, **20.4%** | **4** (a 5th clipped by the tab bar) | ~85 px, **15.0%** |

The WPT Global phone creative is still only evidence of *intent*, and the intent it shows is
unambiguous: banner, one tab strip, one filter row, then the list — **no hero**.

#### 9.4.2 What ClearDeck did, and what it does now

Both rows measured on the real local canisters through the screenshot harness's browser, three
tables registered, signed out.

| capture | viewport | first row at | rows fully visible | data fields |
|---|---|---|---|---|
| before (wave-3 critic) | 1440×900 | y=796, **88.4%** | **1** of 3 | 7 in the row + 18 in the preview |
| after (wave 4) | 1440×900 | y=370, **41.1%** | **3** of 3 | unchanged |
| **after (wave 5)** | 1440×900 | y=312.8, **34.8%** | **3** of 3 | unchanged |
| before (wave-3 critic) | 390×844 | y=1004, **119%** | **0** of 3 | 6 in the card (preview hidden) |
| after (wave 4) | 390×844 | y=543, **64.3%** | **1** of 3 | unchanged |
| **after (wave 5)** | 390×844 | y=484.3, **57.4%** | **2** of 3 | unchanged |

Side by side with the clients the band comes from, so the comparison is not left to a reader:

| client / capture | window | first row at | rows fully visible | fields per row | preview |
|---|---|---|---|---|---|
| PokerStars (desktop) | 732×552 | **34.4%** | 24 | 9 | yes |
| **ClearDeck, wave 5** | 1440×900 | **34.8%** | **3 of 3** (every table it has) | **7** | **yes, 18 fields** |
| ClearDeck, wave 4 | 1440×900 | 41.1% | 3 of 3 | 7 | yes |
| PokerNow (desktop) | 1512×945 | 36.1% | 7 (8th clipped) | 4 | no |
| WPT Global (desktop) | 1127×798 | 26.6% | 5 (every table it had) | 4 | yes |
| PokerStars (phone) | 568 px-tall panel | **20.4%** | 4 (5th clipped) | — | n/a |
| **ClearDeck, wave 5** | 390×844 | **57.4%** | **2 of 3** | **6** | n/a (opens the real table) |
| ClearDeck, wave 4 | 390×844 | 64.3% | 1 of 3 | 6 | n/a |

Desktop is now *inside* the reference range and 0.4 points off its median. The phone is not, and
§9.4.2c below shows why that is arithmetic rather than layout: **45.9% of the phone screen is spent
before `Lobby.svelte` paints a pixel.**

#### 9.4.2a Wave 5: the furniture, itemised

Every number below was read with `getBoundingClientRect()` in the browser the screenshot harness
drives, against the real local canisters, three tables registered, signed out, `scrollY = 0`, with
all four protected notices measured **on screen** in the same pass (§9.4.2c). Scratch harness:
`measure.mjs` / `sweep.mjs` (first-row %, rows fully visible, every horizontally clipped container,
every pill's box, and each protected phrase's own glyph rects tested for viewport containment and
`elementFromPoint` occlusion).

| the lobby's own furniture, top of `.lobby` → top of first row | 1440×900 | 390×844 |
|---|---|---|
| wave 4: pane bar | 56.0 | 84.3 |
| wave 4: drift strip, above the list | 26.2 | 56.8 |
| wave 4: column header row | 34.3 | — (cards) |
| wave 4: padding | 10.9 | 13.9 |
| **wave 4 total** | **127.4** | **155.0** |
| wave 5: pane bar (one row: heading, counts, 0% rake, drift chip, filters) | 35.0 | 78.3–94.8 |
| wave 5: drift strip | 0 (moved below the rows) | 0 (moved below the rows) |
| wave 5: column header row | 30.3 | — (cards) |
| wave 5: padding | 5.0 | 2.0 |
| **wave 5 total** | **70.3** | **80.3–96.8** |

The phone bar has a range because the counts line grows a phrase (`· 2 hands in play`) whenever a
hand is running anywhere in the lobby, and at 366 px that wraps it to a second line. 96.8 is the
worst case and it is the number the table above uses.

Desktop, **−57.1 px**, and the arithmetic closes:

| change | px |
|---|---|
| the drift statement moved from above the rows to below them | **−26.2** |
| the bar itself, 56.0 → 35.0 | **−21.0** |
| column header row, 9 px of padding → 7 | **−4.0** |
| `.lobby` top pad 10 → 4, bar pad 6 → 3 | **−5.9** |
| | **−57.1** |

The bar's own −21.0 is two changes that only work together, which is why they are one line:

* **the heading moved beside the counts** instead of above them, one type step down: the title block
  measures 21.8 px on one baseline against 43.0 stacked;
* **the density and refresh controls moved to the list footer.** They were 121 px of the bar's one
  row, and with the heading inline the row's content came to 890.5 px in an 884 px pane — so it
  wrapped, and the bar measured **63.8**, worse than wave 4's 56. Moving the two controls out is
  what let the row fit on one line. Neither is a control anybody reaches for *before* reading the
  list: Refresh now sits beside the sentence stating the 12-second poll interval it overrides, with
  its word back (the top bar could not afford the 52 px), and row density sits beside the rows it
  changes. Both do exactly what they did.

The drift statement's *count* stayed above the list as a `.drift-chip` inside the counts line, which
costs no height at all and links to the full sentence; each affected row still carries
`⚠ RECORD DIFFERS` in the money cell the stale record misquotes; and the full sentence is
immediately under the rows it is about.

On the phone the card itself came down from **194.6 px to 166.4 px** — `⚠ RECORD DIFFERS` and
`n sitting out` are inline beside the figures they qualify instead of taking a line each. That is
what makes the second table fit: 484.3 + 166.4 + 6 + 166.4 = 823.1 in an 844 px screen.

**Nothing was deleted.** 7 facts in the row, 18 in the preview, 3 live tables signed out, all five
filter pills rendered whole at every width. §9.4.2b is the sweep that says so.

#### 9.4.2b Wave 5: the same measurement at every width the lobby declares

Ten widths, one browser, same replica, same instant. `first row` is `%` of the viewport height;
`clipped` counts containers whose `scrollWidth` exceeds their `clientWidth` **with** a non-visible
`overflow-x`, anywhere on the page, `.bg-effects` excluded (a fixed decorative glow layer in
`+page.svelte`, no content).

| viewport | first row | rows fully visible | pills / lines / cut | clipped | notices on screen |
|---|---|---|---|---|---|
| 390×844 | **57.4%** | 2 of 3 | 5 / 1 / 0 | none | 4 of 4 |
| 430×932 | **46.3%** | 2 of 3 | 5 / 1 / 0 | none | 4 of 4 |
| 500×900 | **47.3%** | 2 of 3 | 5 / 1 / 0 | none | 4 of 4 |
| 760×900 | **39.3%** | 2 of 3 | 5 / 1 / 0 | none | 4 of 4 |
| 900×900 | **37.7%** | 3 of 3 | 5 / 1 / 0 | none | 4 of 4 |
| 1000×900 | **34.8%** | 3 of 3 | 5 / 1 / 0 | none | 4 of 4 |
| 1080×900 | **34.8%** | 3 of 3 | 5 / 1 / 0 | none | 4 of 4 |
| 1240×900 | **37.7%** | 3 of 3 | 5 / 1 / 0 | none | 4 of 4 |
| 1440×900 | **34.8%** | 3 of 3 | 5 / 1 / 0 | none | 4 of 4 |
| 1920×1080 | **29.0%** | 3 of 3 | 5 / 1 / 0 | none | 4 of 4 |

Two robustness cases, both simulated in the page and labelled as simulations:

* **the filter set grows** (three pills injected, standing in for the currency row that appears the
  moment a second currency is registered — [DEFECTS.md T-05](DEFECTS.md#t-05)): at 1440 the eight
  pills still render on **one line, none cut**, first row 37.7%; at 390 they wrap to **two lines,
  none cut**, first row 61.6% and 1 of 4 cards fully visible. BAR 28 holds in both; BAR 31's
  second half does not survive the extra row on a phone, which is a fact the T-05 fix has to carry.
* **the portrait notice strip applied to the lobby** — see the arithmetic below.

#### 9.4.2c Wave 5: the notices, measured on the rendered page

The wave-4 lesson is that `textContent` gates cannot see this, so each protected phrase is located
by walking text nodes, a `Range` is put around the phrase itself, and **its own glyph rectangles**
are tested for (a) full containment in the viewport at `scrollY = 0` and (b) `elementFromPoint` at
each rect's centre resolving back to the phrase's own element.

| phrase | 1440×900 | 390×844 | with the How-it-works dialog open |
|---|---|---|---|
| `Unaudited code with known bugs` | y 18–33 ✓ | y 18–33 ✓ | ✓ both viewports |
| `your funds are NOT safe` | y 36.5–51.5 ✓ | y 54.5–69.5 ✓ | ✓ both viewports |
| `18+ only` | y 54.5–69.5 ✓ | y 108.5–123.5 ✓ | ✓ both viewports |
| `illegal in many jurisdictions` | y 36.5–51.5 ✓ | y 90.5–105.5 ✓ | ✓ both viewports |
| `No middleman, no house` | y 80.5–95.5 ✓ | y 134.5–149.5 ✓ | ✓ both viewports |
| `0% rake` (the lobby's own chip) | y 258.6–272.6 ✓ | y 412.5–426.5 ✓ | behind the dialog; the dialog states `No rake.` at its head, measured on screen ✓ |

**The floor neither the lobby nor this bar can move.** The disclaimer banner and the app header are
`+page.svelte`, and the disclaimer is protected copy. Measured at the same moment as the rows
above, they occupy **160 + 82.5 = 242.5 px of a 900 px desktop viewport (26.9%)** and **268 + 119.5
= 387.5 px of an 844 px phone (45.9%)**. So on a phone *no* lobby layout can reach the reference band:
46.0% is the arithmetic floor before `Lobby.svelte` paints a pixel, and every threshold below is
therefore expressed as **the lobby's own furniture**, which is the only part the lobby owns.

> **CORRECTION (wave-4 coherence pass).** That paragraph charges the protected copy **2.9× what it
> costs.** The 268 px is the whole banner block; the paragraph carrying the four notices
> `make hygiene` enforces (`.banner-warning`) is **109.5 px** on a phone and **55.5 px** on
> desktop. The remainder is `.banner-info` ("No middleman, no house…", 90 px) and `.banner-ai`
> ("This entire project was built 100% by AI", 19.5 px) plus padding, and **no rule protects
> either**. The true protected floor is **109.5 + 119.5 = 229 px = 27.1%** on a phone, not 46.0%,
> and **55.5 px = 6.2%** of a 900 px desktop viewport, not 17.8%. The 20–43% band is therefore
> reachable on a phone without weakening one protected word, and BAR 25's stated derivation and
> BAR 26's 243 px subtraction both inherit the same conflation. Left in place because the bars
> themselves are still where the work should aim; the *reasoning under them* is what is wrong.

> **WAVE 5: the phone deficit is now entirely chrome, and it is 28 px.** With the lobby's own
> furniture at 96.8 px the phone's first card is at 57.4%. Drive the furniture to **zero** and the
> first card still lands at **387.5 px = 45.9%**, which is 3.3 points outside the 42.6% ceiling.
> **No change to `Lobby.svelte` can reach the band on a phone**: 28.0 px has to come out of the
> 387.5 px above it, and every one of those pixels belongs to `+page.svelte` / `index.scss`.
>
> The mechanism to do it already exists in `+page.svelte` and is already argued for there. Wave 5's
> concurrent pass added `.banner-strip` — a compact red strip carrying the protected phrases
> **verbatim** (`Unaudited code with known bugs`, `your funds are NOT safe`, `illegal in many
> jurisdictions`, `18+ only`, plus the no-house/0% rake property), one tap from the full text — and
> scoped it to `class:on-table={view === 'table'}`, with the comment "the lobby in portrait still
> gets the full banner in the flow, because on the lobby nothing is competing for the space".
> On the lobby something is: this bar. Measured with that treatment applied to the lobby at 390×844
> (simulated in the page by adding `on-table` to `.alpha-warning-banner` and `compact` to
> `header` — `+page.svelte` was **not** edited):
>
> | | banner | header | chrome | first card | rows fully visible | notices on screen |
> |---|---|---|---|---|---|---|
> | lobby as it ships | 268.0 | 119.5 | 387.5 | y 484.3 = **57.4%** | 2 of 3 | 4 of 4 |
> | lobby with the strip | 59.6 | 42.0 | 101.6 | y 198.3 = **23.5%** | **3 of 3** | **4 of 4** |
>
> 23.5% is inside the band and within 3.1 points of the PokerStars mobile reference (20.4%), with
> every table on screen and every protected phrase measured on screen and unoccluded. The change is
> one condition in a file this pass does not own; it is written up as
> [DEFECTS.md L-03](DEFECTS.md#l-03).

#### 9.4.3 The bars

> **BAR 25 (restated numerically).** **Desktop: the first table row starts within the top 43% of
> the viewport.** The observed reference range is 26.6% – 36.1%; the bar is set 6.9 points looser
> than the widest of them, and that slack is exactly the price of the protected disclaimer, which
> none of the three reference clients carries and which alone is 17.8% of a 900 px viewport. Note
> what the floor implies: **27.0% is unreachable-from-below even with zero lobby furniture**, so
> the bar is really "spend under 16 points of viewport on furniture". Today: **34.8%** at 1440×900,
> and 29.0%–37.7% across every width from 900 px up (§9.4.2b). Was 41.1% in wave 4 and 88.4% in
> wave 3. 34.8% is 0.4 points off the PokerStars reference (34.4%) and inside the 26.6%–36.1%
> reference range, so the 6.9 points of slack this bar granted for the protected disclaimer are no
> longer being spent. (The wave-3 critic recorded PokerNow at 36.5% for the same capture;
> re-measured here at y≈341 of 945 = 36.1%. Either way it is the widest reference.)

> **BAR 26 (new).** **The lobby's own furniture above the first row is ≤ 140 px at desktop and
> ≤ 160 px at 390 px wide** — heading, counts, filters, controls, any warning strip, and the column
> header row, everything between the app header and the first row. Derived from BAR 25 minus the
> 243 px floor (0.43 × 900 − 243 = 144 px), and applied to the phone as the same absolute budget
> because the furniture does not get cheaper on a smaller screen. Today: **70.3 px** desktop
> (bar 35 + column header 30.3 + 5 px of padding — the drift strip is below the rows now) and
> **80.3–96.8 px** phone (bar 78.3–94.8 + 2 px of padding; the range is a hand running or not,
> §9.4.2a). Was 127.4 / 155.0 in wave 4. Both halves clear with 50 px to spare.

> **BAR 27 (new).** **At least 3 table rows fully visible without scrolling, or every table that
> exists, whichever is smaller.** Three is the smallest number that lets a player *compare* rather
> than read one and scroll; the references do far better (PokerStars 24, PokerNow 7) and WPT Global
> shows all 5 it has. Today: **3 of 3** at desktop (and 3 of 3 at every width from 900 px up),
> **2 of 3** on the phone. Wave 4 read 1 of 3; the card came down from 194.6 px to 166.4 px
> (§9.4.2a) and the furniture from 155 px to 96.8 px, which is what the second card is made of. The
> third needs the chrome fix: with it, 3 of 3 at 390×844 — measured, in the wave-5 note above
> §9.4.3 and in [DEFECTS.md L-03](DEFECTS.md#l-03).

> **BAR 28 (new).** **No filter, tab or control may be clipped at any viewport.** WPT Global renders
> all five stake tabs and all three game tabs at 1127 px; nothing in the corpus scroll-clips a
> filter. Before this pass ClearDeck's phone filter strip measured `scrollWidth 434` against
> `clientWidth 366` under `overflow-x: auto`, slicing "Micro" mid-word. Today the strip wraps:
> `scrollWidth 366 == clientWidth 366`, `overflow-x: visible`, all five pills on one line, **0 cut**
> — re-verified this pass at ten widths from 390 to 1920, and with three extra pills injected, where
> it wraps to a second line and still cuts nothing (§9.4.2b).
>
> **WAVE 5 FINDING: this bar was being failed by something nobody had measured — the row's own
> action control.** At 1440×900 the table's min-content width was 934.5 px inside a 908 px pane and
> `.list-pane { overflow: hidden }` clipped the right 26.5 px, so every row's `Sit` / `View` /
> `Watch` label lost 10.5 px of itself, arrow included. It was not one breakpoint: four separate
> width ranges clipped it (761–848, 1001–1064, 1081–1184, 1241–1288), because which columns fit is a
> question about the PANE's width and every rule deciding it was written against the VIEWPORT's.
> Fixed at the root with `container-type: inline-size` on the pane and two `@container` rules at the
> measured floors (879 px of pane for seven columns, 807 for six, 687 for five), plus 12 px cell
> padding instead of 16. Ten widths now report zero clipped containers.
> [DEFECTS.md L-01](DEFECTS.md#l-01).
>
> A second control was failing it too, in a file the same pass owns: the **How-it-works dialog's own
> close button**, centred on the viewport at `top: 50%` while the banner (`z-index: 100` on `.app`)
> paints over everything inside `<main>` whatever its z-index. Measured at 1440×900 the dialog began
> at y=67.5 under 243 px of chrome: its title row and its `×` were unclickable, and only the keyboard
> path worked. The dialog now measures the chrome and opens under it (253 px at desktop, 398 on a
> phone), which also keeps the four notices out from behind the scrim.
> [DEFECTS.md L-02](DEFECTS.md#l-02).

> **BAR 29 (new).** **A row states at least 4 facts about its table, and the desktop lobby ships a
> master/detail preview.** 4 is the floor set by both PokerNow and WPT Global; the preview is what
> two of the three desktop references ship. ClearDeck states **7** in the row (name, currency +
> format tags, stakes, buy-in range, seat occupancy incl. sitting-out, hands dealt, live phase +
> pot) and **18** in the preview, so this bar is a floor to defend, not a gap. **Compressing the
> lobby must never be paid for with this bar** — the wave-3 rebuild moved furniture and deleted no
> field, and neither did wave 5: 7 in the row and 18 in the preview, re-counted on the rendered page
> at every width in §9.4.2b. What wave 5 does do is make the column set depend on the pane's width
> rather than the viewport's, so `Hands` (and below 807 px of pane, `Buy-in`) drop out where they
> would otherwise have pushed the action control off the row — the buy-in range is still stated in
> the preview's facts list at every width, and the desktop grid at 1440 still ships all seven.

> **BAR 30 (new).** **Every price on the lobby screen comes from the table contract.** Not a
> reference-derived bar — a correctness one, and it is here because it is a lobby-layout trap: the
> lobby canister's registered *name* carries a stakes string (`init_microstakes_tables` bakes
> table_1's blinds into all three names), so a row could read `9-Max - 0.01/0.02` with `0.10/0.20`
> in the Stakes cell 222 px to its right. The name is still quoted verbatim — it is the table's
> registered identity — but any figure inside it that the contract contradicts is struck through
> and flagged, so exactly one figure on the row reads as a price and it is the contract's.
>
> **WAVE 5 AUDIT: every price on the lobby, read off the screen and compared with the chain.**
> Six figures per ICP row and six in the preview, at 1440×900 and 390×844:
>
> | on screen | contract (`get_table_view().config`) | verdict |
> |---|---|---|
> | row `0.10/0.20 ICP`, buy-in `20.00 – 100.00` | table_3 `10_000_000 / 20_000_000`, `2e9 / 1e10` | agrees |
> | row `0.01/0.02 ICP`, buy-in `2.00 – 10.00` | table_1 `1_000_000 / 2_000_000`, `2e8 / 1e9` | agrees |
> | row `0.05/0.10 ICP`, buy-in `10.00 – 50.00` | table_2 `5_000_000 / 10_000_000`, `1e9 / 5e9` | agrees |
> | preview `Blinds 0.10/0.20`, `Buy-in 20.00 – 100.00`, `Clock 60s + 30s`, `Ante None` | table_3, incl. `action_timeout_secs = 60`, `time_bank_secs = 30`, `ante = 0` | agrees |
> | row NAME `9-Max - 0.01/0.02`, `6-Max - 0.01/0.02`; preview heading likewise | table_3 charges 0.10/0.20, table_2 charges 0.05/0.10 | **DISAGREES — 6 figures** |
>
> Every price the client *computes* is the contract's. The six that disagree are all the same
> string: the name the LOBBY canister has registered. The client already strikes it through and
> labels it `STALE NAME`, but the harness reads `textContent` and is right to: a struck figure is
> still a figure on the screen. **This is a data defect, not a rendering one, and it is why the two
> lobby scenes are red.** Fixing it needs one admin update call per table plus one new lobby method
> that does not exist yet; both are specified, with the on-chain evidence, in
> [DEFECTS.md L-04](DEFECTS.md#l-04). No engine file was touched this pass.

> **BAR 31 (new, wave-4 coherence pass).** **Mobile: the first table row starts within the top 45%
> of the screen, and at least 2 rows are fully visible.** Anchored on the one real mobile lobby
> capture in the corpus, PokerStars at **20.4% with 4 rows** (§9.4.1). The bar is set 24.6 points
> looser than the reference because ClearDeck carries 229 px of genuinely protected chrome that
> PokerStars does not (see the correction above); it is *not* set at the reference, because nobody
> should pretend that gap is closable this wave. Today: **first row at y = 484.3 = 57.4%, 2 of 3
> rows fully visible.** The second half now passes. The first half fails by 12.4 points, and wave 5
> establishes that **10.8 of those 12.4 points are not the lobby's to spend**: with the lobby's
> furniture at zero the first card is still at 45.9%. See the wave-5 note under §9.4.2c for the
> 28 px, and [DEFECTS.md L-03](DEFECTS.md#l-03) for the one-condition change that measures 23.5%.

### 9.5 Wave-4 coherence pass: bars superseded, and the mobile table restated

> **BAR 22 is unchanged as a bar (≥ 40% portrait playing surface) and its "today" figure is
> restated.** §9.3 recorded 19.7–21.1%; wave 4 recorded 42.9–52.1%. **Both are wrong for the
> shipping build.** The 42.9–52.1% was measured in a configuration where the four protected
> notices were hidden from the table view, which [DEFECTS.md T-20](DEFECTS.md#t-20) establishes is
> not shippable. Measured on the shipping build with the notices on screen:
>
> | variant | notices on screen | 6-max | 9-max |
> |---|---|---|---|
> | **shipping now** | 4 of 4 | 199.1×358.7 = **18.3%** | 188.2×339.2 = **16.4%** |
> | wave 4 as built | 0 of 4 | 335.4×604.3 = 51.9% | 304.2×548.1 = 42.7% |
> | `3253b67` geometry, notices on screen | 4 of 4 | 238.9×341.3 = **20.9%** | 238.9×341.3 = **20.9%** |
> | PokerNow, real capture | n/a | 313×548 = **52.1%** | — |
>
> Against PokerNow the shipping build is **2.9:1 behind**, and it is a regression on the geometry
> it replaced. The aspect is not the bug — a 0.555 felt is the right shape only when there is
> height to spend, and at a bound height a narrower felt has less area. The vertical budget is the
> bug: [DEFECTS.md T-19](DEFECTS.md#t-19) (0.918 zoom-out, 411.8 px `.header-right`) and the
> 173.5 px three-row table header. Full A/B in [WAVE-04.md §4](WAVE-04.md).

> **BAR 23 (felt wholly inside the viewport) is CLEAR.** Independently re-measured this pass on
> both viewports and at both densities: `offFrame: []` for `.pot-display`, `.community-cards`,
> every `.player-nameplate`, `.action-dock` and `.board-cluster`, with `scrollY = 0` on arrival.

> **BAR 24 (money glyphs ≥ 12 px in portrait) is NOT cleared, and its reference figure is wrong.**
> Measured digit ink is 11–12 px, with two 9-max pods at **11 px**, under the bar's own floor. The
> `16.4 / 14.4 px` figures reported in wave 4 are computed `font-size`, not glyph ink. Separately,
> the bar's reference ("PokerNow portrait's 15 px") is **not reproducible**: independent detectors
> read 8 px on every PokerNow stack and 12 px on its largest money glyph anywhere in that capture.
> Ours does beat PokerNow's stacks by ~50% on ink; it does not clear 12 px everywhere.

> **BAR 32 (new).** **No element may occlude a figure a player acts on.** Not a
> reference-derived bar — a defect-derived one, and the sharpest thing the wave-4 walk found:
> `artifacts/screens/latest/table-showdown-mobile.png` renders the winner's `100.00%` equity as a
> visible **`0%`**, because the hero's own card covers the rest of the badge, and the award chip
> covers the winner's revealed pair ([DEFECTS.md T-22](DEFECTS.md#t-22),
> [T-23](DEFECTS.md#t-23)). A truncated percentage that reads as a plausible different percentage
> is worse than showing nothing. **This bar is only adjudicable in pixels**: every gate in the repo
> reads `textContent` and every one of them called those scenes verified.

### 9.6 Bars this file still cannot adjudicate

Stated so a future reader does not mistake silence for a pass:

* **Motion.** Only PokerNow's timings are known (from its shipped CSS). Nothing in this document
  can judge whether ClearDeck's transitions feel like the category.
* **Sound.** No reference audio was collected.
* **Landscape phone.** One PokerNow capture exists; ClearDeck has no landscape scene.
* **The seated bet-sizing strip.** PokerNow's could not be captured (game creation is behind a bot
  check we will not defeat), so bars 7 and 12 have no PokerNow anchor.

---

## 10. Wave-5 coherence pass, the measurements, corrected and now gated

Everything in this section was measured on the rendered page against the real local canisters, in
the only legal configuration (all five protected phrases on screen and hit-tested), and every
number here is now an **assertion in the harness** rather than a figure in a document:
`tools/shots/lib/felt-area.mjs`, run centrally from `run.mjs` for every scene at every viewport,
records the geometry into `manifest.json` and prints it into a new **felt** column in `INDEX.md`
beside a new **NOTICES** column ([DEFECTS.md H-40](DEFECTS.md#h-40)).

### 10.1 Measure `.felt`, not `.poker-table`: the factor-of-two trap

`.poker-table` is the whole stage: felt, seat pods, action dock. Measuring it at 1440×900 reads
**1400×629 = 68% of the window area** where the playing surface is **31.7%**. The first version of
this pass's own probe fell into it. `.felt` is the layout box of the visible green surface (the rail
is drawn outside it with box-shadow rings so it costs no layout height), and it is the only element
any felt figure in this repository should ever mean.

Note that §9.1's "width %" column is felt width ÷ **window width** (962/1440 = 66.8%), while the
new gate reports felt **area** ÷ window area. Both are useful; neither is the other, and a reader
comparing 66.8% with 31.7% is comparing a length with an area.

### 10.2 The settled geometry, both viewports, as gated

| scene | table | 1440×900 | 390×844 |
|---|---|---|---|
| `table-preflop`, `table-facing-bet`, `table-showdown` | `table_2`, 6-max | 929.3×442.5 = **31.7%** area, **64.5%** width, aspect 2.10 | 332.8×599.5 = **60.6%** area, aspect 0.555 |
| `table-empty`, `table-allin`, `table-sidepots` | `table_3`, 9-max | 950.3×452.5 = **33.2%** area, **66.0%** width, aspect 2.10 | 304.2×548.1 = **50.7%** area, aspect 0.555 |

Floors asserted: **45.0%** of frame area at 390×844, **28.0%** at 1440×900. They are deliberately
loose, a wave-4-scale regression is 18.3%, because a gate that flakes gets switched off, and the
recorded number is what a reader compares wave to wave. Proved in both directions on the shipping
build: changing one declaration in the shipped stylesheet, `--fw: min(86cqw, 55cqh)` →
`min(50cqw, 32cqh)`, drops `table-preflop` mobile to **193.6×348.8 = 20.5%** and the scene goes red
naming the number and the floor.

### 10.3 Correction to §9.1: the desktop felt is 64.5% of window width, not 66.8%

`table-preflop-desktop` and `table-facing-bet-desktop` read 962×452 in wave 5 and read
**929.3×442.5** now. The 18 px of stage height came out of the desktop banner, because this pass
had to put the canonical no-rake sentence somewhere a desktop player can see it
([DEFECTS.md T-36](DEFECTS.md#t-36)): before the fix, *"No rake is taken from any pot on any
table"* was on screen on **one** of the nine surfaces a player can reach, and on none of the
desktop ones.

**Bar 1 (70% ±8 of window width) is still CLEARED**, now at 64.5–66.0% instead of 66.8–68.3%, and
bar 2 (aspect 1.9–2.3) is unchanged at 2.10. The trade is not negotiable under HARD RULE 2 and is
recorded here so nobody has to rediscover where the 18 px went. The right place to buy it back is
the non-protected marketing clause in the same paragraph ("Built to demonstrate the power of the
Internet Computer…"), never a notice.

### 10.4 The portrait numbers reproduce, and they are the SETTLED state

`table-preflop` mobile 332.8×599.5 = **60.6%** reproduces exactly, three times, on three separate
runs and on two independent probes. Against the references in this document: PokerNow's real
portrait capture **49.1%**, GGPoker creative 40.3% (fill SUSPECT), PokerStars creative 27.4%. The
9-max 50.7% is width-capped by `--fw: min(78cqw, 52cqh)` → min(304.2, 314.6) and no vertical budget
moves it.

**What was never distinguished until now: 60.6% is the settled state, and it is not the first
paint.** `PokerTable.svelte:313` falls back to `max_players ?? 9`, so every table, including a
6-max one, first draws a NINE-seat ring, and the felt corrects when the canister answers
([DEFECTS.md T-32](DEFECTS.md#t-32)). Sampled per animation frame on entry to `table_2`:

| viewport | first paint | held | settles to |
|---|---|---|---|
| 390×844 | 287.5×518.1 = **45.3%** rising to 50.7%, 9 pods | **336 ms** | 332.8×599.5 = **60.6%**, 6 pods |
| 1440×900 | 950.3×452.5 = **33.2%**, 9 pods | **304 ms** | 929.3×442.5 = **31.7%**, 6 pods |

That is a visible 9.4% linear jump of the whole table on every phone entry, and it is why the new
gate records the pod count and the ring class beside every number: a felt figure without them does
not say which of the two states it measured. **A new bar follows from it.**

> **BAR 33 (new).** **The table must not resize under the player after it is drawn.** A client that
> guesses the seat count and corrects it a third of a second later has published two different
> tables, and every felt measurement in this document is then ambiguous. The ring must be drawn from
> a known `max_players` — the lobby row the player clicked already carries it.

### 10.5 Bar 32 (nothing may occlude a figure), and the notice it cannot see

Bar 32 is now enforced in pixels by `tools/shots/lib/occlusion.mjs`, and the full run reports **0
occluded** on all 22 shots. Two limits, recorded because a bar whose limits are unwritten reads as
broader than it is:

1. It measures **figures**: money, equity, card rank and pip. It does not measure **notices**,
   which is why nothing in the harness noticed that the desktop table stated the no-rake property
   nowhere a player could see it. That half is now `protected-notices.mjs`, run centrally.
2. It measures figures the DOM scan **nominates as occluders**, so a full cover by an element its
   paint model believes is underneath is not gated. The gate's own critic got five such constructions
   past it. Filed by its author; not re-litigated here.

### 10.6 What this section still cannot adjudicate

The four gaps in §9.6 all stand. One is now sharper: **the harness photographs 390×844 and 1440×900
only**, so the two viewports this wave had to fix by hand, 844×390 landscape phone (felt
400.0×190.5 = 23.1%) and 320×568, have numbers, no bar, and no scene. A felt floor that no scene
evaluates at those sizes is not protecting them.
