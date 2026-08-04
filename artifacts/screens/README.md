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
.thirdparty-cache/               cached Google Fonts / dicebear avatars (STATIC assets only)
```

## How to read a filename — this is the whole trust model

| Filename | What it asserts |
|---|---|
| `<scene>-<viewport>.png` | the scene's on-chain **and** DOM assertions all passed. This name is evidence. |
| `UNVERIFIED-<scene>-<viewport>.png` | the page rendered and was photographed, but the scene's own `verify()` returned false. Read `manifest.json` for the reason before quoting the image. |
| `FAILED-<scene>-<viewport>.png` | the scene threw. Forensic still only. |

A run clears **every** variant of a (scene, viewport) from `latest/` before writing, so a
verified PNG from an earlier commit can never sit in `latest/` beside this run's
`UNVERIFIED-` one and be mistaken for current evidence.

## Fiat figures in these screenshots are never a replayed quote

Fonts and avatars are immutable, so they are cached to disk and replayed — that keeps a run
repeatable and offline-capable and tells no lie. A **price** is not immutable: a CoinGecko
quote read last week is a false statement today, and the app renders it as `~$12.34` next to
a real on-chain balance. So `api.coingecko.com` is never served from the cache. Each run
records in `manifest.json → volatileThirdParty.mode` exactly one of:

- `live` — a real quote, with the timestamp it was read at
- `unavailable` — the feed could not be read, so the app shows its own "no price" state and
  the screenshot contains **no** fiat figure (an honest absence)
- `fixture` — `SHOTS_PRICE_FIXTURE=1` was set; the number on screen is a placeholder, and
  `INDEX.md` says so on its own line
- `none` — no volatile request was made at all

## Repository hygiene: a decision only the lead can make

A full run writes ~36 MB of PNG/WebM. `make hygiene` fails on it, by design — it refuses
untracked binaries and any untracked payload over 4 MiB. The stanza `tools/shots/README.md`
recommends is:

```
artifacts/screens/*
!artifacts/screens/latest/
artifacts/screens/.thirdparty-cache/
tools/shots/node_modules/
```

That keeps only `latest/` reviewable in git and leaves the per-SHA directories local. It is
**not applied** here: `.gitignore` is the lead's call, and every image is reproducible with
one command.

Scenes: `lobby`, `table-empty`, `table-preflop`, `table-allin`, `table-showdown`,
`table-sidepots`, `deposit`, `handhistory`, `shuffleproof`.

`manifest.json` is the thing to read when judging whether a screenshot is trustworthy. For
each scene it records the on-chain state the driver reached (phase, pot, side pots, all-in
seats, winners, hand number, seed hash), the DOM assertions that were checked against it,
the canister IDs the page actually called, and any console/page errors. A scene that could
not be reached leaves `FAILED-<scene>-<viewport>.png` and an `error` string instead.

See `tools/shots/README.md` for how state is produced, how determinism is enforced, and the
`.gitignore` recommendation.
