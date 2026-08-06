# Every money figure the client renders, and whether it equals the chain

Audited 2026-08-05 against the local replica, at `7bc69db` + working tree. Each row is either
gated by `tools/shots` (re-run `node tools/shots/run.mjs` to re-check it) or was read from the
source and is marked as such.

## Gated by the screenshot harness — all agreeing today

| figure | chain source | scene |
| --- | --- | --- |
| headline pot | `get_pot()` | every table scene |
| pot breakdown "X collected + Y betting" | sums to `get_pot()`; second leg is `sum(current_bet)` | table scenes |
| seat stacks | `get_table_view().players[i].chips` | every table scene |
| seat live bets | `players[i].current_bet` | every table scene |
| side pots + their count | `side_pots[i].amount`; also that they sum to `get_pot()` | `table-sidepots` |
| board cards | `get_community_cards()` | every table scene |
| pot-odds ratio | `get_pot() / call_amount` | `table-facing-bet` |
| required-equity % | `call / (get_pot() + call)` | `table-facing-bet` |
| "Call X" on the button and in the turn hint | `call_amount` | `table-facing-bet` |
| ½ Pot / Pot presets — **the amount that would be sent** | `current_bet + get_pot()(/2) + call_amount` | `table-facing-bet` |
| table wallet balance | `get_balance()` | table scenes, `deposit` |
| winner banner amount + seat | `last_hand_winners[..]` | `table-showdown` |
| lobby stakes and buy-in cells | the TABLE canister's live config | `lobby` |
| lobby live pot per row | `get_pot()` on that table | `lobby` |
| deposit modal wallet balance | ledger `icrc1_balance_of` | `deposit` |
| deposit modal fiat value | balance x the quote served in that run | `deposit` |
| hand-history row pot | history `total_pot`, cross-checked against `sum(winners.amount)` | `handhistory` |
| rake, per hand | history `rake == 0` | `handhistory` |

## Discrepancies found

| # | where | what | status |
| --- | --- | --- | --- |
| 1 | `PokerTable.svelte` pot, pot odds, equity hint, bet presets | the 2x pot (DEFECTS T-08) had four faces, not one; the presets **wagered** 2x rather than merely displaying it | FIXED during this wave by the component's owner; now gated |
| 2 | `WithdrawModal.svelte:74` | the success message renders the **ledger block index** as a token amount: `withdraw()` returns `Ok(block_index)` and the modal prints `formatWithUnit(result.Ok)` | OPEN — not my file |
| 3 | `WithdrawModal.svelte` + `transfer_tokens` | the wallet receives `amount - fee`; the modal never states the fee numerically and never shows the net | OPEN — not my file |
| 4 | `lobby_canister::init_microstakes_tables` | all three rows are named "... - 0.01/0.02" and stored with 0.01/0.02 blinds and a 2-10 ICP buy-in, but table_2 enforces 0.05/0.10 (10-50) and table_3 enforces 0.10/0.20 (20-100) | OPEN — gated; `lobby` is red for exactly this |

## Not covered by any scene — open surfaces, listed so silence is not mistaken for a pass

* the action log (`ActionFeed`) bet/raise/win amounts: the drawer is closed in every scene;
* the withdraw modal: no scene opens it;
* the header wallet dropdown balance in `WalletButton` (note it uses the class `.balance-value`,
  the same class `PokerTable`'s wallet panel uses for the **escrow** balance — two different
  quantities under one class name; the scraper scopes to `.wallet-panel` to stay unambiguous);
* the hand-history detail panel's per-winner amounts (only the row pot is checked);
* anything on a **BTC** table: `btc_table_1` is registered in the lobby by no deploy path
  (DEFECTS T-05), so no scene can reach it and the sats formatting path is unexercised.
