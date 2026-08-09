# What an open browser tab costs a table canister — the raw runs

These are the measurement runs behind [docs/DEFECTS.md E-92](../../docs/DEFECTS.md#e-92) and the
corrected runway table in [E-55](../../docs/DEFECTS.md#e-55). They are tracked because the numbers
in the register are differences taken from them, and a published difference whose inputs are not on
disk is a number nobody can check.

Each file is one cell of the matrix, written by `tools/cycles/tab-burn.mjs`:

| file | what it is |
|---|---|
| `control-before.json`, `control-after.json` | **the control.** Zero tabs. The canister's on-chain clock burns cycles whether or not anybody is looking, so every marginal figure is a difference against this. Taken twice, at the start and the end of a 30-minute matrix, and they came out 0.011% apart |
| `legacy-{1,3,10}tab.json` | the 500 ms poll as it was before E-92: `check_timeouts()` — an UPDATE — as the first statement of every poll |
| `fixed-{1,3,10}tab.json` | the poll as it is now: queries only, with the clock advanced by `$lib/clockNudge.js`'s policy. Measured against a table nobody is playing at, which is the flattering case and is labelled as such |
| `fixed-max-{1,3,10}tab.json` | the same policy pinned at its 2 s floor with the table moving on every call: the most one tab can ever cost. `fallback_burn_per_day` is built from this |
| `fixed-stuck-10tab.json` | a table permanently "due" and never moving, so the backoff walks the gap out to 30 s. The pathological case the floor exists for |

Re-run the whole matrix (local replica only, ~30 minutes):

```
./tools/cycles/run-matrix.sh 120
node tools/cycles/build-burn-table.mjs
```

The second command folds these into `tools/cycles/burn-table.json`, which is the single source every
runway figure in the tree reads. `node tools/shots/test-burn-table.mjs` fails if that file stops
recomputing from its own components, if `scripts/cycles-runway.sh` goes back to carrying its own
copy of the numbers, or if a retired figure reappears in code.

The method, and its two stated confounds, are in the header of `tools/cycles/tab-burn.mjs`.
