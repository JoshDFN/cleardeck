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

| Bar | PokerNow | PokerStars | GGPoker | WPT Global |
|---|---|---|---|---|
| Felt colour (sampled) | `#26804D` | `#146127` | `#0B4E10` (green) / `#014F59` (teal) | `#00757D` / `#00737A` |
| Surround / out-of-felt colour | `#242324` | `#17191D` | `#132013` / `#0C1719` | `#080925` / `#04031C` |
| Surround share of the window | 60.3% | 55.7% | 66.4% / **82.5%** | 57.4% / 51.9% |
| Felt aspect ratio (w:h) | **1.60** | ~1.40 | ~1.10–1.32 | **1.80–2.27** |
| Board card width ÷ felt width | **13.4%** (exact) | ~7.0% | not reliably measurable | ~6.5% |
| Seat pods sit… | **off the felt**, on the surround | on the rail | on the rail | on the rail |
| Compliance / responsible-gaming text | bottom strip | persistent "Responsible Gaming" top-left | footer only | footer only |

Three things jump out and they are the real strategic findings:

1. **Every one of these clients is majority dark surround.** The felt is a minority of
   the pixels — between 52% and 82% of the window is *not* felt. Nobody fills the
   viewport with green. GGPoker's teal table is the most extreme: 82.5% of the window is
   near-black `#0C1719`. A poker client reads as premium because the table is a lit
   object floating in a dark room.
2. **Felt aspect ratio splits the field.** WPT Global stretches to 1.8–2.27 (a long
   stadium). GGPoker is nearly round (1.1–1.3). PokerNow sits at 1.60. This is a real
   design choice, not a detail: a wide table buys horizontal room for 9–10 pods, a round
   table concentrates attention on the board.
3. **PokerNow's board is roughly twice as large, proportionally, as the paid clients'.**
   A single board card is 13.4% of felt width on PokerNow versus ~6.5–7% on WPT Global
   and PokerStars. The five-card board spans **72.2% of the felt width** on PokerNow.
   That is a legibility-over-realism decision and it is why PokerNow reads well on a
   phone.

---

## 2. PokerNow — measured exactly

PokerNow is the reference we can measure to the pixel, so it carries the most quotable
bars. All values are CSS pixels at a **1512 × 945** viewport unless stated.

### 2.1 Layout geometry (exact, from DOM `getBoundingClientRect`)

| Element | Size | Notes |
|---|---|---|
| `.table` (whole playing area incl. surround) | 1511.8 × 714.4, top at y=47.3 | 100% of window width, **75.6% of window height** |
| Visible felt ellipse | **912 × 570** | **60.3% of window width, 60.3% of window height**, aspect **1.60** |
| Board card (`.card`, "big") | **122.5 × 149.7** | aspect **0.818**; 13.4% of felt width |
| Five-card board (`.table-cards`) | 658.6 × 149.7 | **72.2% of felt width** |
| Opponent hole card ("med") | 83.0 × 100.7 | **68% of a board card** — opponents' cards are deliberately smaller |
| Seat pod (`.table-player`) | **272.1 × 96.4** | 29.8% of felt width; **outside** the ellipse |
| Player name / stack rows | 122.5 × 24.5 / 103.4 × 21.8 | name above stack, left-aligned in the pod |
| Pot readout (`.table-pot-size`) | 196.5 × 51.7 | centred, **above** the board, 21.5% of felt width |

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

**Table and layout**
1. Playing surface occupies **55–65% of window width** at desktop, with the rest as a
   dark surround. Do not fill the viewport with felt.
2. Felt aspect ratio in **1.5–1.9** for a 6–9 seat table. Below 1.4 wastes horizontal
   space; above 2.3 only works with a heavy rail.
3. Surround must be genuinely dark (`#17191D`–`#242324` band) and must be the **majority
   of the pixels**.
4. Empty seats render as a **full-size pod with an explicit call to action**
   (`SIT` / `Sit Here` / `+`). Never a gap.
5. Seat pod width **25–30% of felt width**. Name above stack, two rows, left-aligned.

**Cards**
6. Board card aspect **0.80–0.82**. `border-radius` ~7% of width.
7. Board card width **≥7% of felt width**; PokerNow's 13.4% is the legibility ceiling and
   the right target for a browser-first client. The five-card board should span
   **50–72% of felt width**.
8. Opponents' hole cards render **smaller than board cards** (PokerNow: 68%).
9. Card rank in a display face at **~2× base UI size**, red `#DB3131`-ish, black
   `#2C2C2C` (never `#000`).
10. **Two typefaces total** on the table: one UI sans, one display face for ranks.

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
    called out. GGPoker adds a named hand-strength phrase. ClearDeck should show live
    equity at all-in — it is the single highest-value thing three of four leaders do and
    it costs no animation budget.

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
| **No PokerNow seated bet-sizing controls** | We observed as a spectator and chose not to sit down in strangers' live games. | Bet-control geometry for PokerNow unmeasured; PokerStars' and GGPoker's are covered from creatives. |
| **No real-gameplay all-in for WPT Global** | Its only all-in reference is an illustrated marketing render. | Do not treat WPT's all-in composition as authoritative. |
| **Board-card ratio not measurable for GGPoker** | Available captures were too small / the board too obscured for reliable white-rectangle segmentation. | GG card proportions are qualitative only. |
| **No screen recordings for the three native clients** | Frame-accurate motion timing needs video; we have exact CSS timings only for PokerNow. | All motion numbers in §6 derive from PokerNow. Treat them as a floor validated on one client, not four. |
| **Felt bounding boxes for the native clients are ±5%** | Colour segmentation on third-party screenshots that include window chrome. | Only PokerNow's geometry is exact. |
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

```sh
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
