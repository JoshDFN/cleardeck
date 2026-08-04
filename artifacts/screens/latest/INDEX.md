# ClearDeck screenshot run

- commit: `801aa79` (working tree dirty)
- started: 2026-08-04T20:52:41.996Z
- auth: the app's own local-dev login (WalletButton 'Dev Login' -> auth.devLogin), a deterministic Ed25519 identity; every canister call is really signed by it. Local Internet Identity is not available on this replica (network descriptor has ii:false).
- app origin: http://127.0.0.1:4943
- gateway: http://localhost:8077
- frontend asset canister: 5uljf-s3777-77775-aaaea-cai
- fiat figures: **live** — every fiat figure in this run is a real quote, dated below

| scene | viewport | file | state verified on-chain | notes |
| --- | --- | --- | --- | --- |
| lobby | desktop | `artifacts/screens/801aa79/lobby-desktop.png` | yes | 3 live table rows; signed out; lobby state="ready"; loading spinner absent |
| lobby | mobile | `artifacts/screens/801aa79/lobby-mobile.png` | yes | 3 live table rows; signed out; lobby state="ready"; loading spinner absent |
| table-empty | desktop | `artifacts/screens/801aa79/table-empty-desktop.png` | yes | 8 open seats, hero seated, phase "Waiting For Players" |
| table-empty | mobile | `artifacts/screens/801aa79/table-empty-mobile.png` | yes | 8 open seats, hero seated, phase "Waiting For Players" |
| table-preflop | desktop | `artifacts/screens/801aa79/table-preflop-desktop.png` | yes | action buttons: Fold / Check / Raise / All In / +30s; action clock visible |
| table-preflop | mobile | `artifacts/screens/801aa79/table-preflop-mobile.png` | yes | action buttons: Fold / Check / Raise / All In / +30s; action clock HIDDEN at this viewport |
| table-allin | desktop | `artifacts/screens/801aa79/table-allin-desktop.png` | yes | 2 ALL IN badges rendered, pot "POT 220.40 (110.20 + 110.20 betting)" |
| table-allin | mobile | `artifacts/screens/801aa79/table-allin-mobile.png` | yes | 2 ALL IN badges rendered, pot "POT 220.40 (110.20 + 110.20 betting)" |
| table-showdown | desktop | `artifacts/screens/801aa79/table-showdown-desktop.png` | yes | You won 24.00 ICP! 0 |
| table-showdown | mobile | `artifacts/screens/801aa79/table-showdown-mobile.png` | yes | You won 24.00 ICP! 0 |
| table-sidepots | desktop | `artifacts/screens/801aa79/table-sidepots-desktop.png` | yes | 2 side pots rendered, 2 ALL IN badges, phase Flop |
| table-sidepots | mobile | `artifacts/screens/801aa79/table-sidepots-mobile.png` | yes | 2 side pots rendered, 2 ALL IN badges, phase Flop |
| deposit | desktop | `artifacts/screens/801aa79/deposit-desktop.png` | yes | Deposit ICP to Table |
| deposit | mobile | `artifacts/screens/801aa79/UNVERIFIED-deposit-mobile.png` | NO | deposit modal UNREACHABLE at 390px: the only trigger is inside .feed-container, hidden below 900px. This is an app defect, not a staging failure. |
| handhistory | desktop | `artifacts/screens/801aa79/handhistory-desktop.png` | yes | 1 hand row(s); first: Hand #1 8/4/2026, 8:54:36 PM 2 players Pot: 24.00 ICP You won! (Pair) Showdown |
| handhistory | mobile | `artifacts/screens/801aa79/handhistory-mobile.png` | yes | 1 hand row(s); first: Hand #1 8/4/2026, 8:54:49 PM 2 players Pot: 24.00 ICP You lost Showdown |
| shuffleproof | desktop | `artifacts/screens/801aa79/shuffleproof-desktop.png` | yes | 3 proof rows, revealed seed shown |
| shuffleproof | mobile | `artifacts/screens/801aa79/shuffleproof-mobile.png` | yes | 3 proof rows, revealed seed shown |
