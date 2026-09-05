# The UI/UX wave: what changed and what is open

Six phases on `feat/ui-ux-wave`, one brief: "just improve our current UI/UX".
No new renderer, no Unreal, no Three.js, no rewrite. The angled top-down table
and the measured geometry in DESIGN-BAR.md stayed (felt aspect 2.1, pods on
the rail, board cards 6.5-13% of the felt width, 500 ms object moves, 150 ms
flips). Everything else on the screen was allowed to move, under one hard
product rule: the five protected notices stay visible, in the viewport and
unoccluded on every scene at both viewports, because the product is an
unaudited alpha with open fund-safety defects.

The wave was driven by a 76-finding audit across seven lenses (the table
scene, the decision loop, mobile ergonomics, the lobby and trust, the cashier,
the fairness surfaces, and the type / colour / motion / sound system), each
finding with screenshot evidence, a reference to what the leading clients do,
and a fix. The BEFORE stills are `artifacts/screens/eb819a8/`; the closer's
full AFTER run is `artifacts/screens/5be78af/` (both viewports, every scene,
gating mode). The Rust canisters were out of scope and are byte-identical to
`main` (section 7).

The numbers this wave moved, from the harness's own INDEX rows:

| measure | before (eb819a8) | after (5be78af) |
| --- | --- | --- |
| felt share of the frame, desktop 6-max showdown | 31.7% | 41.0% |
| felt share, desktop 9-max (sidepots) | 30.7% | 44.3% |
| felt share, desktop pre-flop on the hero's turn | 27.8% (under the 28% floor, UNVERIFIED) | 41.0% |
| felt share, phone 9-max all-in | 42.9% (under the 45% floor, UNVERIFIED) | 48.3% |
| chrome above the felt, desktop | 322 px | 146-185 px |
| first lobby card, desktop / phone | 39.7% / 57.4% of the viewport (audit) | 35% / 43% |
| smallest desktop action button | 57.5x42 (under 44) | 76x44 |
| smallest phone touch target outside the action row | 24-36 px (audit) | 44 px, gated |
| the deposit sheet | UNVERIFIED (8 unasserted tokens), amount below the fold | verified, amount first |
| scenes verified in gating mode | 20 of 24 | 30 of 30 |
| frontend unit tests | 0 | 383 (35 files) |
| `--cd-*` design tokens declared / used | 30 / 0 | 164 / load-bearing everywhere on the table, lobby, cashier and fairness surfaces |
| lines: PokerTable.svelte / +page.svelte / HandHistory / ShuffleProof / Lobby / DepositModal | 3842 / 2989 / 1829 / 1536 / 2471 / 2753 | 2261 / 2653 / 570 / 758 / 713 / 798 |

## 1. What changed, per phase

Each phase ran three rounds (build, critic, fix) and finished with a gating
harness run of its scenes at both viewports. The per-round detail (every
failure mode a run found and what fixed it) is in the harness manager's
HANDOFF; this is the durable summary.

### Phase 1: the table scene and the design system

- `src/index.scss` is a real token layer now, and it is load-bearing: one
  teal (`--cd-accent*`), one money gold (`--cd-money*`, #eeb441), one danger
  red, one warn amber, felt and rail ramps, card and chip colours, an on-felt
  type scale in em of `--ui` floored at 11 px, a 4 px spacing grid, four
  radii, shadows, motion tokens (`--cd-move` 500 ms, `--cd-flip` 150 ms), a
  global reduced-motion rule, and self-hosted fonts (Inter variable, Playfair
  Display 700 for the ranks; the Google `@import` is gone).
- The room: the lobby's animated blur orbs are hidden on the table view and
  replaced by one static gradient; the stage is a lit room with a contact
  shadow under the rail. The rail is its own element behind the felt (charcoal
  padded roll, near rail 2.5x the far rail, lit crown); the felt is a deep
  desaturated green with a key light, fibre noise, a two-falloff vignette, a
  betting line and one spade mark. The orange all-in recolour is gone.
- Objects: real card faces (corner index, mirrored index, centre pip, sheen,
  cast shadow), a proper card back, 150 ms flip, staggered board deal; bets as
  CSS chip stacks with the amount in a gold capsule; the dealer button as a
  puck on the felt that travels 500 ms; SB/BB badges deleted; empty seats as a
  solid disc with `+ Sit`; local SVG monogram avatars (no dicebear fetch on
  the table).
- Pods as broadcast lower-thirds (`SeatPod`, `HoleCards`, `BetStack`,
  `DealerPuck`, `EquityBadge`, `WinnerAward`, `EmptySeat`): avatar disc
  breaking the plate's left edge, name row, stack row, the action clock as a
  conic ring, the hero's named hand as the plate's third row. The felt carries
  only cards, chips, the puck and one pot module (`PotModule`: POT + street +
  one gold figure; side pots one row only when they exist; the breakdown and
  the equity method demoted to a title and a LOG line). Winner readout one
  gold line naming the seat.
- Per-seat placement is pure geometry in `lib/table-geometry.js` (puck spot,
  readout spoke, revealed-cell shift, free spoke end, portrait award spot),
  unit-tested, published as custom properties on `.seat`.
- The five notices became ONE neutral trust bar on the table view (amber
  glyph, 13 px, FULL TERMS opens the complete text); the header is one 52 px
  bar with one button primitive. Desktop felt went from 31.7% to 41.0% (6-max)
  and 44.3% (9-max) of the frame.
- The giants split: `poker-table-tokens.scss`, `poker-table-dock.scss`,
  `table-header-phone.scss` as Sass mixins included at the cascade position
  the rules used to occupy.

### Phase 2: the decision loop

- `lib/optimistic.js`: the click paints the hero's action before the chain
  answers (a frozen `pendingAction` record; the stack debited, the chips slide
  out, a `.plate-tag.sent` on the plate, the row in `.sent` with the pressed
  button reading "Calling 0.10", the clock frozen); every poll reconciles it
  (`pendingStatus`: absorbed when the hand, street or hero record moved on;
  open otherwise, with `is_my_turn` held false so the dock cannot flicker);
  12 s TTL; a humane message on Err; a throw says the action may have landed.
  No `tableState` field is written in place any more.
- `lib/pre-actions.js` + `PreActions.svelte`: Check / Fold, Check, Call any
  (nothing owed) or Fold to any bet, Call X, Call any (facing a bet), armed
  while waiting, shown on the hero plate, fired on the certified my-turn edge,
  dropped when the amount to call or the hand changes.
- `lib/bet-sizing.js` + `BetSizer.svelte`: sizing IN THE DOCK (never over the
  felt): street-aware presets (pre-flop Min / 2.5x / 3x / 4x / Pot / All in;
  post-flop Min / half / two-thirds / Pot / All in), a slider stepping one
  display unit, +/- one big blind, a typed field, every figure quantised to
  the display grid and clamped to the canister's floor and cap; "Raise to X"
  commits in one click. On the phone the sizer is an opaque sheet above the
  action row that never reflows the felt.
- `ActionBar.svelte`: underlined hotkeys (F / C / R / A A / 1-6 / + -), the
  single `resolveHotkey()` decision in `lib/hotkeys.js` (Enter and Space are
  never shortcuts; nothing fires behind a dialog or from a control outside the
  dock), press and focus states, a 2 px clock line with an urgent state, the
  inline `.action-error` strip (6 s), the seconds on the primary button under
  10 s.
- `lib/action-clock.js`: local interpolation, resync only when the chain
  disagrees by more than 2 s, frozen while a send is open. `lib/turn-alert.js`
  + `use-turn-alert.svelte.js`: a two-note chime, vibration on touch, the tab
  title marked, once per turn identity (a same-street re-raise fires again);
  opt-out in the wallet menu. `TimeBankPill` under the turn indicator
  (desktop) or on the hero pod (phone), only in the last 15 s, never in the
  action row. The poll ran at 250 ms through the wave; the closer put it back
  to 500 ms (section 4: `tools/cycles/tab-burn.mjs` prices an open tab at the
  period the page runs and `burn-table.json` was measured at 500 ms, so
  `test-poll-updates.mjs` is red on any drift; a faster poll needs the burn
  table re-measured first). The hero's own action is re-read the moment its
  update returns regardless of the period.

### Phase 3: mobile ergonomics at 390x844

- The phone table header is ONE 44 px row (back, stakes, History and Verify
  as glyphs, an avatar-only wallet chip; signed out, one Connect button); the
  dock's Log, Sound, Deposit, Withdraw are 44 px; empty seats carry a 44 px hit
  area; the toast's close is 44 px; `touch-action: manipulation` everywhere;
  safe-area insets; a PWA manifest (display `browser`, not `standalone`:
  section 8); `overscroll-behavior-y: none` on a live hand.
- The table page is ONE screen in portrait (100dvh, footer off the table
  view, its links in the Log drawer). The hero pair is 60 px with a 6 degree
  fan (50 px on the 9-max phone); an opponent's clock digits sit on the acting
  avatar so a long name never ellipsises; the clock line is 4 px.
- The join gate (`lib/join-gate.js`, certified facts only): an anonymous Sit
  opens sign-in and remembers the seat; escrow under the table's own
  `min_buy_in` opens the deposit sheet on the exact shortfall; the canister's
  balance string never reaches the screen.
- The money sheets are full-height on the phone (fixed header, one scroller,
  scroll lock behind, `lib/scroll-lock.js`); the amount is on the first
  screen; the Deposit row pins to the sheet's foot only after the disclosures
  have scrolled past (`lib/pin-after.js`), because the security placement
  rule (SECURITY-FINDINGS 23 / 35 / 42) puts every disclosure before a control
  that moves money and the deposit scene measures it.
- Landscape under 560 px tall shows "Turn your phone upright" over the table
  area only; the notices and polling stay.
- `tools/shots/touch-targets.mjs`: a gate that measures the EFFECTIVE hit area
  of every control (elementFromPoint at the centre and 21 px out), the type
  floors, the scroll width, the scroll lock, the toast's close, and sticky
  rows over money; 18 states clean.

### Phase 4: the lobby, first run and trust

- `TrustBar.svelte` carries the five phrases on EVERY view at every viewport,
  sticky, one line at 13 px on desktop, three lines at 11 px on the phone,
  FULL TERMS opening the complete text as an opaque overlay. The footer's
  duplicate disclaimer became a neutral provenance line. The five phrases are
  typed once in `lib/notices.js` and rendered by every surface through
  `NoticeLine` / `NoticeStrip` (six hand-synced copies gone).
- `LobbyHero`: "Poker you can check." + the claims line (0% rake, SHA-256
  committed deck, on-chain contracts) + "Sign in with Internet Identity" with
  a one-line hint; the first card lands at 35% of the desktop viewport and
  43% of the phone's with the cold-start row up. `LobbyTableRow`: tables as
  cards (name and format tags, blinds with the unit and a fiat hint, buy-in
  with a fiat hint, a seats meter, the clock, status, Watch / Sit, Invite).
  `LobbyColdStart`: one row when nobody is seated (Open the cheapest ICP
  table, Copy invite link). `LobbySteps`: three steps to a seat.
- `lib/prices.js` (one cached CoinGecko read) and `lib/lobby-format.js`: fiat
  hints under every stakes and buy-in cell, the Micro / Low pills defined in
  dollars; every fiat figure asserted by the harness against amount x the
  served quote.
- Deep links: `?table=<canister id>` opens the table (`lib/invite-link.js`);
  opening a table writes it to the URL, Back clears it. "Connect Wallet" reads
  "Sign in"; the mainnet chip is neutral; the drift footnote is quiet and off
  the money cells. A humane lobby failure (`lib/humane-errors.js`): sentence,
  detail line, Retry, back-off 12 / 24 / 48 / 60 s; `Toast.svelte` extracted;
  `LobbyEmpty` for the four empty states; a spectator dock ("Sign in to sit")
  instead of a wallet panel for an anonymous viewer.

### Phase 5: the cashier

- `DepositModal` rebuilt for a depositor: one route per state (from this
  wallet when it can pay the minimum plus both fees; from an exchange or
  another wallet otherwise, with a QR address card that watches its own
  subaccount every 5 s and sweeps what arrives by itself), the paying wallet
  with its balance and fiat, the amount with quick chips carrying the table's
  own minimum buy-in, MAX, a cost summary (you send / two ledger fees / total
  from your wallet / the table credits), the limits line, the disclosures
  organised beside it (custody headline with How, destination, solvency with
  the canister's sentence under a disclosure, runway), the two chain commits
  painted as steps tuned to the 1.75 s commit, success a receipt with Done.
  The OISY path no longer disconnects on success and names who paid and who
  is credited.
- `WithdrawModal`: the balance with fiat, MAX, the net rows, the destination
  account id derived from the signed-in principal (copyable), the stepper, a
  receipt with the ledger block and the cooldown counting down on the button.
- `lib/cashier-format.js` (one formatter per context, four decimals with the
  unit in the cashier, the exact e8s once), `lib/cashier-steps.js`,
  `describeCashierFailure` (every canister and ledger refusal as a sentence
  ending "Nothing moved." with the raw text as the detail), `lib/qr-svg.js`,
  `lib/deposit-*.js` modules (wallet reader, flow, submit, detect, receipts,
  amounts), `CashierStepper`, `CashierSummary`, `CashierReceipt`,
  `CashierDisclosures`, `DepositAddressCard`, `DepositAmountField`,
  `DepositRouteTabs`, `QuickAmounts`. DepositModal is 798 lines (was 2753 on
  main); WithdrawModal 728.
- Two probes move REAL local money and check every figure on the ledger:
  `probe-cashier.mjs` (deposit, withdraw, the cooldown refusal, the phone
  button within two screens with an amount typed) and
  `probe-deposit-address.mjs` (drains a wallet, pays the derived address from
  a second identity, watches the detection and the sweep).

### Phase 6: the fairness surfaces

- `ShuffleProof` is verdict-first: one card (the seal glyph, the proven
  sentence, your cards derived = dealt with real faces, the board cell by
  cell, the computed seal snapping onto the committed one, the limit as a
  tag), then the four rungs, the raw hashes, the deck grid, the independent
  verifiers and the proven / not-proven table behind disclosures, every honest
  word kept. Hashes everywhere are fingerprints (first 8 + last 8) beside a
  deterministic 5x5 seal glyph (`lib/hash-seal.js`, `HashSeal.svelte`); the
  full hex is behind Copy.
- `HandList`: a scannable list (result, your cards, board, pot, opponents,
  time, the seal) with four filters; a stable hand id (`6_max#1`) with Copy;
  `?table=<canister>&hand=N` opens the replay directly (`lib/hand-link.js`).
  "Copy as text" exports the PokerStars dialect with the seed hash and the
  revealed seed as trailing comments (`lib/hand-text-export.js`).
- `HandReplayer` opens on a mini table (`ReplayTable`: felt, board, pot
  module, lower-third pods, bet chips, the puck), one stop per ACTION
  (`lib/replay-stops.js`: the pot and street bets folded exactly from the
  record, never invented), the street scrubber, play at the house tempo, the
  log line lit and kept in view (`ReplayLog`), equity on every pod where every
  live hand is known and a 4-point equity line under the transport
  (`lib/replay-equity-line.js`, `ReplayEquityLine`), labelled exact or Monte
  Carlo, asserted by the harness against an independent oracle. On desktop the
  replay is a 1280 px broadcast picture (a ~600 px felt, 60 px board cards).
- The seal on the felt: `DeckSeal.svelte`, a stack of card backs at the
  near rail's corner carrying the commitment's glyph (Sealed, Revealed,
  Checked once THIS browser hashed the revealed seed), click opens the proof;
  the action log's "VRF -> SHA256" pill is the same state machine as a chip
  (`lib/deck-seal.js`); the emoji icons became colour-coded verbs.
- The LOG is a surface the table yields to: a 200 px column beside the felt on
  desktop (felt 29.8% with it open, above the floor), a 26dvh shade under the
  header over the far seats on the phone, `role=region` so the hotkeys keep
  firing with it open (a DOM-fixture test pins the markup). The math modules
  (`shuffle-verify.js`, `commitment-witness.js`, `equity.js`,
  `hand-record.js`) are byte-identical to main.

### The closer

- `lib/sounds.js`: the audio graph is built on the first play and resumed on
  the first gesture (the AudioContext used to be constructed at import time,
  which every browser's autoplay policy leaves `suspended`, so on mainnet no
  beep and no your-turn chime was ever heard: audit finding 45, critical);
  `soundManager.state` reports unsupported / idle / suspended / running;
  6 unit tests. No pixel changed.
- `PotModule.svelte`: a split-pot readout on the 6-max phone is three rows,
  not four, and sits closer to the board (the first full run dealt a split
  and the readout covered 10% of a flank seat's equity badge).
- `routes/+page.svelte`: the poll back to 500 ms (the burn table's period;
  section 4). `PokerTable.svelte`: the base `.wallet-committed` rule back in
  the component ahead of the dock partial, because
  `tests/money_safety/tests/ui_limits.rs` reads that file for it.
  `tools/shots/test-dock-overflow.mjs`: compiles the component's Sass (with
  the token layer) instead of reading the style block as CSS, its pins
  re-measured against the wave's dock.

## 2. Before and after

Every pair is the same scene, the same viewport, the same harness. BEFORE is
the audit's run at `eb819a8`; AFTER is the closer's full run at `5be78af`.

| scene | before | after |
| --- | --- | --- |
| lobby, desktop | `artifacts/screens/eb819a8/lobby-desktop.png` (250 px red banner, first card at 39.7%) | `artifacts/screens/5be78af/lobby-desktop.png` (one trust bar, hero, cold start, cards with fiat, first card at 35%) |
| lobby, phone | `artifacts/screens/eb819a8/lobby-mobile.png` | `artifacts/screens/5be78af/lobby-mobile.png` |
| table, showdown, desktop | `artifacts/screens/eb819a8/table-showdown-desktop.png` (flat rail, seven text rows down the centre, dashed seats, robots, felt 31.7%) | `artifacts/screens/5be78af/table-showdown-desktop.png` (padded rail, one winner line, lower-third pods, the deck seal, felt 41%) |
| table, showdown, phone | `artifacts/screens/eb819a8/table-showdown-mobile.png` | `artifacts/screens/5be78af/table-showdown-mobile.png` |
| table, facing a bet, desktop | `artifacts/screens/eb819a8/UNVERIFIED-table-facing-bet-desktop.png` (felt 27.8%, a dock button off frame) | `artifacts/screens/5be78af/table-facing-bet-desktop.png` (the sizer in the dock, hotkeys, pot odds, felt 39.8%) |
| table, facing a bet, phone | `artifacts/screens/eb819a8/table-facing-bet-mobile.png` | `artifacts/screens/5be78af/table-facing-bet-mobile.png` (Fold / Call / Raise + caret / All in at 44 px, the fanned pair) |
| table, pre-flop, desktop | `artifacts/screens/eb819a8/UNVERIFIED-table-preflop-desktop.png` | `artifacts/screens/5be78af/table-preflop-desktop.png` |
| table, pre-flop, phone | `artifacts/screens/eb819a8/table-preflop-mobile.png` | `artifacts/screens/5be78af/table-preflop-mobile.png` |
| table, all-in, desktop | `artifacts/screens/eb819a8/table-allin-desktop.png` (orange ring, +30s in the action row at 42 px) | `artifacts/screens/5be78af/table-allin-desktop.png` |
| table, all-in, phone | `artifacts/screens/eb819a8/UNVERIFIED-table-allin-mobile.png` (felt 42.9%) | `artifacts/screens/5be78af/table-allin-mobile.png` |
| table, side pots, desktop | `artifacts/screens/eb819a8/table-sidepots-desktop.png` | `artifacts/screens/5be78af/table-sidepots-desktop.png` |
| table, side pots, phone | `artifacts/screens/eb819a8/table-sidepots-mobile.png` (the pot pill over Ada's clock) | `artifacts/screens/5be78af/table-sidepots-mobile.png` |
| table, empty, desktop | `artifacts/screens/eb819a8/table-empty-desktop.png` | `artifacts/screens/5be78af/table-empty-desktop.png` |
| table, empty, phone | `artifacts/screens/eb819a8/table-empty-mobile.png` | `artifacts/screens/5be78af/table-empty-mobile.png` |
| deposit, desktop | `artifacts/screens/eb819a8/UNVERIFIED-deposit-desktop.png` (four warning blocks, no input in frame) | `artifacts/screens/5be78af/deposit-desktop.png` (route, wallet, amount, cost, disclosures beside it, the button in frame) |
| deposit, phone | `artifacts/screens/eb819a8/UNVERIFIED-deposit-mobile.png` | `artifacts/screens/5be78af/deposit-mobile.png` |
| hand history, desktop | `artifacts/screens/eb819a8/handhistory-desktop.png` (badge soup) | `artifacts/screens/5be78af/handhistory-desktop.png` (cards, board, pot, id with Copy) |
| hand history, phone | `artifacts/screens/eb819a8/handhistory-mobile.png` | `artifacts/screens/5be78af/handhistory-mobile.png` |
| hand replay, desktop | `artifacts/screens/eb819a8/handreplay-desktop.png` (two prose panels, the board clipped) | `artifacts/screens/5be78af/handreplay-desktop.png` (the mini table, per-action stops, equity line, the log lit) |
| hand replay, phone | `artifacts/screens/eb819a8/handreplay-mobile.png` | `artifacts/screens/5be78af/handreplay-mobile.png` |
| shuffle proof, desktop | `artifacts/screens/eb819a8/shuffleproof-desktop.png` (a 3.9-viewport ladder) | `artifacts/screens/5be78af/shuffleproof-desktop.png` (the verdict card) |
| shuffle proof, phone | `artifacts/screens/eb819a8/shuffleproof-mobile.png` | `artifacts/screens/5be78af/shuffleproof-mobile.png` |
| toast over the notices, desktop / phone | `artifacts/screens/eb819a8/toast-notices-{desktop,mobile}.png` | `artifacts/screens/5be78af/toast-notices-{desktop,mobile}.png` |
| NEW scenes (no before) | | `table-waiting` (the pre-action row), `table-facing-bet-flop` (post-flop presets), `table-log` (the seal and the log open) at both viewports under `artifacts/screens/5be78af/` |

Probes (not gates) with their own stills under `artifacts/screens/<sha>/probe/`:
the mid-send echo and the open phone sizer (`probe-decision-loop.mjs`), the
time bank (`probe-time-bank.mjs`), the join gate's three branches
(`probe-join-gate.mjs`), the lobby's toast / terms / scroll / deep link /
failure states (`probe-lobby.mjs`), the cashier with real money
(`probe-cashier.mjs`, `probe-deposit-address.mjs`), the heads-up ring
(`probe-heads-up.mjs`), the 9-max ring at a showdown and a four-seat replay
(`probe-nine-max.mjs`).

## 3. The audit: resolved and open

76 findings: 8 critical, 26 high, 33 medium, 9 low. "Resolved" means the
finding's fix is on the branch and photographed or unit-tested; "partial"
means the frontend half is done and the rest belongs to another lane or a
later wave; "open" means not built.

| severity | resolved | partial | open |
| --- | --- | --- | --- |
| critical (8) | 8 | 0 | 0 |
| high (26) | 21 | 2 | 3 |
| medium (33) | 27 | 6 | 0 |
| low (9) | 7 | 1 | 1 |
| total (76) | 63 | 9 | 4 |

The partial and open items, by lens (numbers are the audit's order):

- Fairness: none open. (8: the list row shows Won / Lost with the pot and no
  invented net figure, because the record carries the award, not the
  contribution.)
- Money flows: **14 (high) OPEN**: a buy-in step and rebuy need
  `join_table(seat, amount)` and `add_chips` wired in the canister lane; the
  join gate's deposit-shortfall prefill is the frontend half. **16 (high)
  partial**: one formatter per context is in (table 2 dp, cashier 4 dp with
  the unit, the exact e8s once) but the dock balance carries no fiat hint.
  **20 (medium) partial**: the drift marker is a quiet footnote off the money
  cells; the mainnet records themselves need the controller's
  `update_table_name` (Josh). **22 (low) OPEN**: the II popup names the raw
  canister origin until the app has a domain, `.well-known/ii-alternative-origins`
  and `derivationOrigin` (Josh; the loading label is fixed).
- Decision loop: **26 (high) OPEN**: timeout semantics (auto-check when
  nothing is owed, auto time bank, sit-out after three) are the canister's;
  the client mirrors the rule in the Check / Fold default. **27 (medium)
  partial**: the time bank is out of the action row and offered only under
  15 s; auto-engaging it on expiry is the canister's.
- Lobby and trust: **39 (high) OPEN**: the branded sign-in needs the domain
  (as 22). **41 (medium) partial**: the red REAL FUNDS pill is a neutral
  chip; the proof rail (escrow = ledger, runway, wasm hash) in the preview
  pane is not built (every figure needs a chain site first). **44 (low)
  partial**: HowItWorks opens with the one-sentence pitch; the Money / Fair
  deal / If something goes wrong reorder and a header entry point are not.
- Type, colour, motion, sound: **48 (high) partial**: the
  table, lobby, cashier and fairness files carry no colour literal, but
  `+page.svelte` (44 hex, the verify dialog and footer), `WalletButton.svelte`
  (42) and `HowItWorks.svelte` (16) still do. **51 (medium) partial**:
  `transition: all` is gone from the table files; 16 remain in WalletButton,
  +page, HowItWorks and ConnectionStatus. **50 (medium) partial**: the header
  is one primitive and the table's avatars are local, but the wallet chip
  still fetches its avatar from dicebear.
- Table scene: none open. (62: the wallet chip's dicebear fetch, as above.)
- Mobile: **72 (medium) partial**: landscape is a rotate prompt, not a
  layout.

## 4. The gates and their results

The closer's run: `node tools/shots/run.mjs` (every scene, both viewports,
gating mode, real canisters on the local replica) at `5be78af`.

- **30 of 30 scene-viewports verified, exit 0**: lobby, table-empty,
  table-preflop, table-facing-bet, table-facing-bet-flop, table-waiting,
  table-allin, table-showdown, table-sidepots, table-log, deposit,
  handhistory, handreplay, shuffleproof, toast-notices, each at 1440x900 and
  390x844. Verdict table: `artifacts/screens/5be78af/INDEX.md` (mirrored to
  `artifacts/screens/latest/`).
- On every one of the 30: chain agreement (631 money figures across the
  run, every one equal to the canister's, the ledger's or amount x the served
  live quote), token census 0 unaccounted (25 allowlist rules), pixel gate 0
  occluded, 5/5 protected notices at rest AND under the error toast, every
  dock button fully in frame.
- Felt share: desktop 39.8-44.3% on the table scenes (floor 28%; 29.8%
  recorded, not asserted, with the log column open and 22.7% beside the
  proof panel); phone 45.5-61.6% (floor 45%). Smallest action button 76x44
  on desktop, 44x48 on the phone. The lobby's first card at 29% / 36% of the
  viewport with players seated (35% / 43% with the cold-start row up).
- The closer's FIRST full run (`bef392e`) was 29 green and one red: table-log
  mobile dealt a split pot and the four-row portrait winner readout covered
  10% of a flank seat's equity badge. Fixed in `5be78af` (section 1, the
  closer) and the whole suite re-run green. The showdown outcome is random
  per run, so the split readout itself is not in the final stills: the fix is
  by measured geometry (about 23 px shorter against a 2-3 px overlap).
- After `cfefe95` (the poll period, the `.wallet-committed` rule's file, the
  dock gate) a gating subset at both viewports, `artifacts/screens/cfefe95/`:
  table-preflop, table-facing-bet, table-waiting, table-allin,
  table-showdown, 10 of 10 verified with felt figures identical to the full
  run; `artifacts/screens/latest/` mirrors the full run at `5be78af`.

Static gates, same tree:

- `npm run check` (svelte-check): 0 errors, 4 pre-existing warnings.
- `DFX_NETWORK=local VITE_CANISTER_ID_LOBBY=... VITE_CANISTER_ID_HISTORY=...
  npm run build`: green.
- `npm test` (vitest): 383 tests in 35 files, green.
- `node tools/shots/test-census.mjs` 24/24 (25 allowlist rules, at the cap),
  `test-touch-targets.mjs` 13/13, `test-occlusion.mjs` 16/16.
- `make test` (the money-safety fast gate, no replica). On this laptop the
  gate needs `POCKET_IC_BIN=~/.cache/dfinity/versions/0.31.0/pocket-ic`: the
  default binary (0.32.0, server 13) is refused by the `pocket-ic` 11 crate
  every Rust harness pins, so a bare `make test` fails every PocketIC test
  before it starts (an environment fact, not the branch's). With the 11.0.0
  server: suite wiring, Candid bindings, `cargo test --workspace`, the wasm
  build, the differential subset, the settlement oracle (78 s) and its
  pinned reproducers, the archive analyser (39/39), the no-peeking spike and
  the custody transcript are GREEN. Two screenshot-harness self-tests were
  red in that run and are green after the closer's `cfefe95`:
  `test-poll-updates.mjs` (the page polled at 250 ms while
  `tools/cycles/tab-burn.mjs` prices the tab at 500 ms and `burn-table.json`
  was measured there; the poll is 500 ms again) and `test-dock-overflow.mjs`
  (it read the component's style block as CSS and the block is Sass with
  mixins now; it compiles it). `node tools/shots/selftest.mjs`: every
  self-test green. RED, and still red: three of the 22 money-safety targets,
  all of them gates that READ FRONTEND SOURCE for a money door by file name:
  `deposit_surface::the_oisy_transfer_destination_is_derived_locally_and_not_fetched`,
  `deposit_trust_root::the_deposit_modal_derives_only_through_the_trust_root`
  and `deposit_trust_root::every_money_door_in_the_modal_refuses_an_unpinned_table`.
  They look in `DepositModal.svelte` for the OISY transfer's `subaccount:
  [X]` with `const X = depositSubaccount(`, for `deriveTrustedDepositAddress(`,
  and for `loadBtcDepositAddress()` with its `!tableIsTrusted` return before
  `get_btc_deposit_address`. The cashier phase moved those doors, statement
  for statement, into `lib/deposit-flow.js` (`depositViaOisy`, lines 40-90:
  `depositSubaccount(sessionPrincipal)` then `subaccount:
  [depositSubaccountBytes]`), `lib/deposit-wallet.svelte.js` (line 55,
  `deriveTrustedDepositAddress(tableCanisterId, principal)`) and
  `lib/deposit-btc.svelte.js` (line 45, `if (!isTrusted()) { ... return }`
  before the fetch), and DepositModal's own `handleDeposit()` still refuses
  an untrusted table with a `return` before `submitDeposit`. So the doors
  are guarded and the gates are blind to the files they moved to, which is
  the gates working, not the doors failing. The two honest fixes are the
  money-safety owner's call: point the three assertions at the module files
  (about twenty lines in `tests/money_safety/tests/`), or veto the split and
  move the three doors back into the modal (about 160 lines, over the
  800-line cap). The UI wave edited neither the tests nor the doors.
  `ui_limits` (the fourth source-reading gate, `.wallet-committed`) was red
  for the same reason and is green again (the closer's `cfefe95`).
- `git diff --stat main..feat/ui-ux-wave` outside `src/cleardeck_frontend`,
  `tools/shots`, `artifacts` and `docs`: two files, both deliberate (section
  7). Nothing under `src/*_canister`, `Cargo.*` or `icp.yaml` changed.

The harness gates this wave added or widened, all in `tools/shots`:

- felt share (28% desktop / 45% mobile floors) recorded rather than asserted
  while a dialog, the proof panel or the log surface is on screen;
- the token census with a 25-rule allowlist cap (every non-money number on
  screen names its rule);
- chain agreement on bet presets (the exact quantised formulas), the hero
  plate tag, the sizer field and button, the pre-action Call figure, lobby
  fiat hints and clock cells, the deposit cost rows, quick chips, detected
  deposits, the solvency row and its exact e8s, the replay's pot and chips at
  every street stop, every equity badge and line cell against an independent
  oracle, every action-log amount against the record;
- touch targets (44 px effective hit area, type floors, no page scroll on the
  table, the scroll lock, sticky rows over money);
- the hotkeys with the log open, the log's surface role and what sits under
  it, the deck seal's state transitions and fingerprint.

## 5. Gotchas

The load-bearing ones; every phase's own list is in the HANDOFF.

- The five phrases live in `lib/notices.js` and nowhere else; the trust bar
  renders them on every view. `scripts/dev.sh hygiene`'s "notices not
  weakened since ceacc37" step diffs whole LINES against a pre-wave baseline
  and is red on the lines the single-sourcing removed; the pixel gate is
  5/5 on every check. The step's owner should re-baseline
  (`CLEARDECK_BASELINE=a6ddd7a` shows the honest diff) or count phrases.
- The Sass partials are MIXINS included where the rules used to sit. A plain
  `@use` emits the rules first and the later base rules win the cascade (the
  phone header went to three rows, measured).
- `svelte.config.js` has `vitePreprocess({ script: false, style: { configFile:
  false } })`; drop `configFile: false` and svelte-check loads vite.config.js,
  whose network guard aborts without `DFX_NETWORK`.
- The token allowlist is AT its cap of 25 rules. A new non-money numeral on a
  gating scene needs a dead rule retired first, or a home inside an existing
  selector.
- `tests/money_safety/tests/ui_limits.rs` reads both money modals as copy,
  `<style lang="scss">` included: no limit word beside a digit outside the
  mirrored fence, so no `min-width: 0` in a modal's style block.
- A sticky row inside a padded scroller must not carry a negative bottom
  margin (Chrome keeps the stuck box inside the content edge: the 24 px lift).
- The log surface must never be `role="dialog"` (the hotkeys die;
  `hotkeys.dom.test.js` pins it). The whole `.action-dock` is the hero's
  control surface for the hotkeys; a control that should mute them belongs
  outside it.
- `.headline` in ShuffleProof is the verdict only; the scene reads the panel
  the moment it exists. Controls the harness drives (the compare field) may
  not fold; prose may.
- `node tools/shots/run.mjs` by absolute path from the repo root; the probes
  and `touch-targets.mjs` run AFTER a run.mjs has deployed the tree, never
  concurrently (they share table_1 / table_2 / table_3).
- E-74: a `local-up --reset` can leave the lobby's admin as the CLI's default
  identity and the lobby empty; the recovery is `set_admin` as that identity
  then `local-up --no-frontend`. T-11: after any re-provision run
  `./scripts/dev.sh local-lobby-sync` or the lobby scene fails chain
  agreement on the stored records.

## 6. What the next wave should do

First, the one thing that is red today and needs an owner: **the three
money-safety gates that read `DepositModal.svelte` by file name** (section
4). Either re-point them at `lib/deposit-flow.js`, `lib/deposit-wallet.svelte.js`
and `lib/deposit-btc.svelte.js`, or move those three doors back into the
modal. Until one of the two is done, `make test` is red on
`deposit_surface` and `deposit_trust_root` while every door they exist to
watch is guarded.

Then, in order of what a player would feel first:

1. **The canister half of the decision loop** (Rust lane): auto-check on
   expiry when nothing is owed, auto-engage the time bank before folding, sit
   out after three timeouts with the count reset on any action;
   `player_action` returning the post-action view (or certified_data) so the
   optimistic echo and the pre-action gate reconcile against a certified
   reply; `using_time_bank` reflected in the view after `use_time_bank`; the
   anonymous reject as the first line of `join_table`.
2. **A buy-in step and rebuy**: `join_table(seat, amount)` + `add_chips` in
   the UI (a slider min..max with the BB equivalent, "Add chips" in the seated
   dock). The frontend's join gate already opens the deposit sheet on the
   shortfall.
3. **Own domain + branded sign-in** (Josh owns the domain):
   `.well-known/ic-domains`, `ii-alternative-origins`, `derivationOrigin` in
   auth.js. Nothing on the lobby survives the id.ai popup naming the raw
   canister until then.
4. **Real sound design**: the graph resumes now; the tones are still
   oscillator beeps. Eight sampled sounds under 150 KB through the existing
   `loadSounds` map, a "tap to enable sound" affordance on the header speaker
   while `soundManager.state` is `suspended`, hotkeys as an opt-in setting
   beside the alert toggle. If the poll is to be faster than 500 ms again,
   re-measure `tools/cycles/burn-table.json` at the new period first
   (`tools/cycles/run-matrix.sh` on the local replica) and change
   `tab-burn.mjs` and the page together.
5. **The residue outside the wave's lanes**: `+page.svelte` (2653 lines, 44
   hex literals in the verify dialog and footer, 4 `transition: all`),
   `WalletButton.svelte` (1679 lines, 42 hex, 10 `transition: all`, the
   dicebear avatar fetch), `HowItWorks.svelte` (16 hex; the Money / Fair deal
   / If something goes wrong reorder and a header entry point),
   `PokerTable.svelte` (2261 lines, geometry and state; the ring computation
   and the flights could move out).
6. **The proof rail** in the lobby preview (escrow = ledger, runway, wasm
   hash per table), each figure with a chain site; a seated-and-waiting player
   by name in the lobby; a fiat hint on the dock balance.
7. **Harness**: a BTC scene (sats sizing is unit-tested only), a drifted-record
   probe with re-sync, `touch-targets.mjs` and the probes folded into
   `run.mjs` (about 15 minutes more per run), a landscape scene when a
   landscape layout exists, an `document.title` probe for the alert.
8. **Phone residue**: the landscape layout (a 22 px strip, icon overlays, a
   one-row dock, 100dvh); the phone Deposit button with an amount typed sits
   at y 1653 (within the asserted two screens) because the solvency block and
   the custody paragraph are product copy; the phone log is a shade over the
   far seats because no yielding layout keeps the felt, the dock and a legible
   log at once on 390x844 (measured twice).

## 7. Outside the frontend: two deliberate diffs

`git diff --stat main..feat/ui-ux-wave -- . ':!src/cleardeck_frontend'
':!tools/shots' ':!artifacts' ':!docs'` lists two files:

- `scripts/dev.sh` (+52): `local-up` now runs `lobby_sync_records`
  (`refresh_all_table_configs`, read back and fatal on a non-Ok reply) so a
  freshly provisioned stack's lobby records say what the contracts charge
  (T-11); a `local-lobby-sync` command re-runs it on a live stack; and a
  hygiene step that fails on any em-dash in the frontend source (the wave's
  typographic rule, excusing the regex character class in `lib/utils.js`).
  Without the sync the lobby scene fails chain agreement after every
  `local-up`; it is the harness working, not the client.
- `package-lock.json` (+624): the root lock for the npm workspace, carrying
  the frontend's new devDependencies (`vitest`, `happy-dom`) and the one new
  dependency (`qrcode-generator`, the deposit address QR).

Nothing under `src/*_canister`, `Cargo.*`, `icp.yaml` or `.icp/` changed.

## 8. After the wave: the review's fixes

The post-wave review (2026-09-05) found the items below; each is fixed on
`feat/ui-ux-wave` in its own commit, with the test that pins it.

- **The PWA is not a standalone app.** `app.html` and
  `static/manifest.webmanifest` shipped `display: standalone` and
  `apple-mobile-web-app-capable=yes`. Sign-in is a popup handshake: the
  Internet Identity window (`lib/auth.js`, `authClient.login`) and the OISY
  signer hand the delegation back through `window.opener` / `postMessage`,
  and a page installed to the iOS home screen in standalone mode gets a
  `window.open` with no opener and no message channel, so the popup can never
  return the delegation and sign-in silently never completes. The manifest
  says `display: "browser"` now (icons and theme colour unchanged) and the
  Apple meta is gone: that meta alone puts an installed page into the same
  opener-less shell whatever the manifest says. The reason is in a comment in
  `app.html` beside the manifest link. A standalone install needs a
  redirect-based sign-in flow first (the II `derivationOrigin` work in
  section 6, item 3).
- **Hotkeys are opt-in, off by default.** `ActionBar.svelte`'s window
  keydown shipped always-on, so with body focus a stray F, C, R or 1-6 folded,
  called or committed a raise. The preference is `lib/hotkeys-pref.js`
  (`poker_hotkeys_enabled`, only the literal `true` enables; a missing key, a
  stale value or a storage that throws all mean off), held live in
  `lib/hotkeys-pref.svelte.js` so the wallet-menu toggle (beside the your-turn
  alert, under Table alerts) is in force on the next key press, and read as
  the FIRST guard in `resolveHotkey` (`enabled !== true` is null before
  anything else is looked at). The key-hints legend under the action row is
  the discovery surface: while the shortcuts are off it reads "Keyboard
  shortcuts off. Turn on in the wallet menu, under Table alerts." (no digit,
  so the token census's `.key-hints` rule is untouched); on, it lists the
  keys as before. The harness's `table-facing-bet` hotkey probe turns the
  preference on through the same localStorage key (plus a `storage` event
  the rune listens for) before its one arming press and off again before the
  still, so the photographed legend is the default state.
