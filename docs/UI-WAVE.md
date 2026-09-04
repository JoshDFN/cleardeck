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
- **The acting flank plate's long name on the phone** ellipsises when the
  clock digits show ("Nakam... 38s"), pre-existing from phase 1; the mobile
  phase owns it.
- **A perf-style probe for the alert.** The your-turn chime, vibration and
  tab title are verified by unit tests and code path; a probe could assert
  `document.title` on the certified edge the way probe-decision-loop.mjs
  photographs the echo.

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
