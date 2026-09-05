# The UI/UX wave: follow-ups the wave filed but did not build

The wave's brief was "improve the current UI/UX": no new renderer, no rewrite,
the felt kept at the measured geometry in DESIGN-BAR.md. Each phase's handoff
lives with the harness manager; this file is the durable list of what the
wave decided NOT to do inside its own lane, so nothing filed in a handoff is
lost when the handoffs are.

## Frontend follow-ups

- **Hotkeys as an opt-in setting.** F / C / R / A A / 1-6 / + - are live by
  default on pointer devices (ActionBar.svelte, lib/hotkeys.js). PokerStars
  ships hotkeys OFF and lets the player switch them on; the key-hints legend
  under the action row is the natural discovery surface, and the wallet
  menu's "Table alerts" section (WalletButton.svelte) the natural home for
  the toggle beside the your-turn alert. Not done in the wave: the legend is
  visible, the keys never fire while a field or a modal has focus, and the
  reviewer's Enter/Space finding is closed.
- **A BTC scene in the harness.** Sats sizing (quantum 1, decimals 0) is
  unit-tested only (bet-sizing.test.js); no scene photographs btc_table_1.
- **The acting flank plate's long name on the phone** ellipsised when the
  clock digits showed ("Nakam... 38s"). The mobile phase moved an opponent's
  digits onto the acting avatar in portrait (SeatPod.svelte); the hero's
  plate keeps them in the row.
- **A real landscape layout for a phone held sideways.** The mobile phase
  ships a "turn your phone upright" panel over the table area only
  (RotatePrompt.svelte, coarse pointer under 560 px tall; the trust bar and
  the header stay on screen and the table keeps polling). The proper fix is
  the audit's: `100dvh` wrapper, a 22 px strip, icon overlays, a one-row
  dock, no page scroll. Portrait is one screen now (routes/app-phone.scss:
  the on-table shell is 100dvh, the footer is off the phone table view and
  its two links ride the Log drawer). Nothing photographs landscape in
  run.mjs; the touch-target script measures it at 750x342.
- **The phone cashier's Deposit button under the disclosures.** DepositModal
  and WithdrawModal are full-height sheets on a phone (fixed header, one
  scroller, 44 px controls, scroll lock behind, money-sheet-phone.scss), and
  the phone's first screen is now the five protected phrases as a compact
  strip, the amount field with its quick chips (the table's minimum buy-in
  and twice it, QuickAmounts.svelte) and the wallet balance. The Deposit
  button stays BELOW the custody, network, solvency and runway disclosures:
  docs/SECURITY-FINDINGS.md FINDING 23 / 35 / 42 put each of them before
  every control that can move money, and tools/shots/scenarios/deposit.mjs
  measures it geometrically at rest (the solvency block's bottom edge above
  the primary button), so a row pinned from the first screen fails that
  scene (measured twice: the first sticky attempt, and the mobile phase's
  third round). What ships: the row pins to the sheet's foot only once the
  solvency block and the runway panel have scrolled above the line its top
  would sit on (src/lib/pin-after.js, `shouldPin` tested; `.actions.pinned`
  in DepositModal's phone block), and the minimum-deposit notice is two
  figure-bearing sentences with a Why disclosure holding the rest verbatim.
  The custody paragraph is the remaining height between the amount and the
  button; a shorter custody paragraph is a security-copy call, not a UI one.
- **Sound for an anonymous spectator on a phone table.** The phone's table
  header has no slot for the sound toggle; it rides the dock beside Log,
  which renders for everyone, signed in or not. If the dock ever drops for
  spectators, the toggle needs another home.
- **A perf-style probe for the alert.** The your-turn chime, vibration and
  tab title are verified by unit tests and code path; a probe could assert
  `document.title` on the certified edge the way probe-decision-loop.mjs
  photographs the echo.

## Lobby, first run and trust (phase 4 follow-ups)

- **Own domain and a branded sign-in.** Internet Identity's dialog still
  says "Sign in to Internet Identity to continue to <canister>.icp0.io"
  because auth.js passes no `derivationOrigin` and static/ has no
  `.well-known/ic-domains` or `ii-alternative-origins`. Register a domain on
  the asset canister, list the raw origin, pass `derivationOrigin` so
  principals stay stable. No lobby polish survives that popup until then.
- **A proof rail in the preview pane.** solvency.js and cycleRunway.js are
  only mounted in the money dialogs; the preview pane could state
  "escrow = ledger, runway N days, wasm <hash>" per table for a visitor
  deciding whether the site is alive. Every figure it would render is money
  or a duration and needs a chain-agreement site before it ships.
- **`scripts/dev.sh hygiene` "notices not weakened"** diffs whole lines
  against ceacc37 and is red on the footer's former copy of the disclaimer
  (moved, verbatim, into TrustBar.svelte's FULL TERMS overlay) and on the
  banner paragraph's em-dash. A phrase-level check belongs to the script's
  owner; the rendered-pixel gate (protected-notices.mjs) is 5/5 everywhere.
- **The cold-start panel with players seated elsewhere.** It shows only when
  every table is empty; a lobby with one table waiting for a second player
  says so on the row ("Waiting for one more") but has no headline nudge.
- **HowItWorks** still leads with cryptography; the audit wants Money / Fair
  deal / If something goes wrong tabs and a header entry point.

## Canister follow-ups (Rust lane, out of the wave's scope)

- **Timeout semantics.** The canister auto-folds on expiry even when a check
  is free, and two timeouts sit a player out. The audit's fix: auto-check
  when nothing is owed, auto-engage the time bank before folding, sit out
  only after three timeouts with the count reset on any action. The client
  mirrors the rule only in the "Check / Fold" pre-action default and in the
  time-bank pill offered in the last fifteen seconds (TimeBankPill.svelte).
- **`player_action` returning the post-action view** (or the canister
  setting certified_data), so pre-action gating and the optimistic echo can
  reconcile against a certified reply instead of the next best-effort poll
  (lib/optimistic.js `pendingStatus`, lib/pre-actions.js `resolvePreAction`).
- **Stale `side_pots`** on a pre-flop hand with no all-in: the canister's
  side pots render faithfully (three pills on a three-handed table), and the
  phone pot module now tightens its column to keep them off the dealer puck
  (PotModule.svelte `tall-column`); the figures themselves are the canister's
  defect (docs/DEFECTS.md).
