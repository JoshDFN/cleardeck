# ClearDeck screenshot run

- commit: `fcfa4e8` (working tree dirty)
- started: 2026-08-05T22:41:57.378Z
- auth: the app's own local-dev login (WalletButton 'Dev Login' -> auth.devLogin), a deterministic Ed25519 identity; every canister call is really signed by it. Local Internet Identity is not available on this replica (network descriptor has ii:false).
- app origin: http://127.0.0.1:4943
- gateway: http://localhost:8077
- frontend asset canister: 5uljf-s3777-77775-aaaea-cai
- fiat figures: **none** — no volatile third-party request was made in this run (no fiat figure on screen)

| scene | viewport | file | agrees with chain | money figures | tokens on screen | chain-matched | allowlisted | UNASSERTED | figures on screen | COVERED | NOTICES | felt | notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| shuffleproof | desktop | `artifacts/screens/fcfa4e8/shuffleproof-desktop.png` | yes | 11 | 236 | 9 | 227 | 0 | 37 | 0 | 5/5 | 24.8% (821.6x391.2, 6 pods) | verdict reached: 7/7 cards re-derived in the browser, hole cards K♥ 6♥ derived = dealt = on the felt, 52-slot deck, computed hash equals the canister's commitment; chain agreement: 11 money figures on screen all equal the canister's; token census: 236 numeric tokens on screen, 9 matched to a canister figure, 227 allowlisted non-monetary (17 rules), 0 unaccounted for; pixel gate: 37 money/equity/card figures on screen, 37 with something overlapping them, 103 pairs pixel-tested, 0 occluded; 5/5 protected notices on screen; felt 821.6x391.2 = 24.8% of frame, aspect 2.1, 6 pods (default), RECORDED not asserted (.shuffle-proof is on screen) |
| shuffleproof | mobile | `artifacts/screens/fcfa4e8/shuffleproof-mobile.png` | yes | 11 | 204 | 9 | 195 | 0 | 34 | 0 | 5/5 | 60.6% (332.8x599.5, 6 pods) | verdict reached: 7/7 cards re-derived in the browser, hole cards K♦ 7♥ derived = dealt = on the felt, 52-slot deck, computed hash equals the canister's commitment; chain agreement: 11 money figures on screen all equal the canister's; token census: 204 numeric tokens on screen, 9 matched to a canister figure, 195 allowlisted non-monetary (17 rules), 0 unaccounted for; pixel gate: 34 money/equity/card figures on screen, 34 with something overlapping them, 341 pairs pixel-tested, 0 occluded, 34 behind an open dialog (not gated); 5/5 protected notices on screen; felt 332.8x599.5 = 60.6% of frame, aspect 0.555, 6 pods (default), RECORDED not asserted (.shuffle-proof is on screen) |
