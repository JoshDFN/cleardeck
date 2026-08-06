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
  manifest.json                  the whole working-out. NOT IN GIT (see below)
  verdicts.json                  TRACKED: one row per (scene, viewport) — the verdict
                                 and the headline of every failure
  INDEX.md                       TRACKED: human-readable table of every shot
latest/                          stable mirror of the newest full run
acknowledged-reds.json           TRACKED: every red in latest/verdicts.json, filed
                                 under a defect. `./scripts/dev.sh shots-verdict` gates
                                 on the pair
.thirdparty-cache/               cached Google Fonts / dicebear avatars (STATIC assets only)
```

## What is in git and what is not (docs/DEFECTS.md E-60)

`manifest.json` used to be tracked on the argument that it is "the evidence about the PNGs".
It is not evidence, it is the working-out: every pixel sample, every occluder pair, every
classified token. When per-figure occlusion sampling arrived it went from **763,703 to
7,612,380 bytes in one wave**, `make hygiene` went red, and ~23 MB of machine-generated JSON
no human has ever read was sitting in a public history. It is now gitignored and stays on
disk next to the PNGs it describes.

**Nothing about a verdict was lost.** `verdicts.json` is the same run's fourteen reds with
their messages in **10,767 bytes**, and `node tools/shots/backfill-verdicts.mjs` regenerates
one from any `manifest.json` still on disk. `make hygiene` now fails on any tracked file over
512 KiB under `artifacts/`, so the growth cannot come back quietly.

## A recorded red is not allowed to be walked past

The sweep exits 1 on a red scene, and that was not enough: the sweep needs a live replica, the
replica was unstartable for two waves, and a real regression (8.7% of a money figure painted
over by the felt, docs/DEFECTS.md E-63) sat in an artifact for a whole wave with **no gate
anybody could run** being red about it.

So `./scripts/dev.sh shots-verdict` — which `make hygiene` runs, and which needs no replica —
reads `latest/verdicts.json` and fails on any red that is not written down in
`acknowledged-reds.json` under a defect that exists in `docs/DEFECTS.md`. It **also** fails on
any entry there that is no longer red, so the ledger cannot rot into a list of permanent
excuses.

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

## Repository hygiene

A full run writes ~36 MB of PNG/WebM plus a ~7.6 MB manifest. `make hygiene` refuses untracked
binaries and any added payload over 4 MiB, and the PNGs, videos and manifests are all
gitignored, so a run leaves the gate green. What a run ADDS to git is `INDEX.md` and
`verdicts.json` per directory: about 36 KB.

Scenes: `lobby`, `table-empty`, `table-preflop`, `table-allin`, `table-showdown`,
`table-sidepots`, `deposit`, `handhistory`, `shuffleproof`.

`manifest.json` is the thing to read when judging whether a screenshot is trustworthy — if you
still have it. For each scene it records the on-chain state the driver reached (phase, pot,
side pots, all-in seats, winners, hand number, seed hash), the DOM assertions that were checked
against it, the canister IDs the page actually called, and any console/page errors. A scene
that could not be reached leaves `FAILED-<scene>-<viewport>.png` and an `error` string instead.
If the run is not yours, `INDEX.md` and `verdicts.json` are what survived into git, and they
carry every verdict and every failure message.

See `tools/shots/README.md` for how state is produced, how determinism is enforced, and the
`.gitignore` recommendation.
