# artifacts/screens

Output of `node tools/shots/run.mjs`. Everything in here is generated — delete it and one
command reproduces it.

```
<git-short-sha>/                 one directory per commit the harness ran against
  <scene>-desktop.png            exactly 1440x900
  <scene>-desktop-full.png       full page
  <scene>-mobile.png             exactly  390x844
  <scene>-mobile-full.png
  motion/table-showdown/
    table-showdown-allin-to-showdown.webm
    frames-desktop/frame-000.png …    frame burst through the same transition
  manifest.json                  per-scene on-chain state + DOM checks + runtime wiring
  INDEX.md                       human-readable table of every shot
latest/                          stable mirror of the newest full run
.thirdparty-cache/               cached Google Fonts / dicebear avatars / price ticker
```

Scenes: `lobby`, `table-empty`, `table-preflop`, `table-allin`, `table-showdown`,
`table-sidepots`, `deposit`, `handhistory`, `shuffleproof`.

`manifest.json` is the thing to read when judging whether a screenshot is trustworthy. For
each scene it records the on-chain state the driver reached (phase, pot, side pots, all-in
seats, winners, hand number, seed hash), the DOM assertions that were checked against it,
the canister IDs the page actually called, and any console/page errors. A scene that could
not be reached leaves `FAILED-<scene>-<viewport>.png` and an `error` string instead.

See `tools/shots/README.md` for how state is produced, how determinism is enforced, and the
`.gitignore` recommendation.
