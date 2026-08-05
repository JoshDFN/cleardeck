# money-truth — the T-08 before/after and the fault-injection proof

The JSON in this directory is the evidence: for each run it records the
canister's own numbers, the numbers scraped off the rendered page, and the
verdict for every money figure. It is text on purpose, so it is diffable and
reviewable in a pull request.

The PNGs those runs produced live in `artifacts/screens/money-truth/<run>/`,
because `.gitignore` keeps every screenshot out of the repository (the evidence
about a screenshot is committed; the screenshot itself is regenerable).

| directory | what it is |
| --- | --- |
| `t08-before/agreement.json` | the 2x pot defect, live, against the real local canisters. Reproducible: rebuild the frontend with `totalPot = pot + liveBets` into a scratch tree and serve it with `SHOTS_SERVE_DIST=<dist> node tools/shots/run.mjs --scenes table-facing-bet --skip-build --skip-deploy` |
| `t08-after/agreement.json` | the same scene, the same hand shape, the deployed build |
| `fault-injection/` | `SHOTS_INJECT_DRIFT=board,sidepot`: the chain untouched, the rendered numbers deliberately falsified, the scene refused the canonical filename |
