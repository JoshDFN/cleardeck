# `tools/archive` — the offline hand-history analyser

An analysis tool over the ClearDeck archive that runs on **your** machine, over data
**you** fetched, and never asks you to trust ours.

The normative write-up, including what each number means and what it does not, is
[`docs/ARCHIVE.md`](../../docs/ARCHIVE.md). This file is the map of the code.

```
fetch/            the ONLY files that open a socket
  fetch-archive.mjs   pulls hand records out of an archive canister -> a JSON bundle
  history-wire.mjs    this tool's own Candid mirror of the archive canister
lib/              the analysis. NO dependencies: node:crypto and nothing else
  cards.mjs           the card model (SHUFFLE-SPEC section 2)
  spec-shuffle.mjs    the shuffle, reimplemented from SHUFFLE-SPEC section 3
  spec-deal.mjs       the dealing order, from SHUFFLE-SPEC section 4
  bundle.mjs          strict validation of a bundle at the boundary
  reconstruct.mjs     seed -> every card, checked against the record
  betting.mjs         the betting replayed, and the money cross-checked
  evaluator.mjs       an independent 7-card hand evaluator
  equity.mjs          exact run-out enumeration (sampled only when it must be)
  pots.mjs            side-pot layering
  ev.mjs              all-in EV, hindsight equity, the exact price of a fold
  stats.mjs           VPIP / PFR / aggression / showdown / by position
  statistics.mjs      Wilson, Fisher, Benjamini-Hochberg, bootstrap, power
  collusion.mjs       the three signals, their sample floors and their refusals
  analyse.mjs         the pipeline, and what it excludes
bin/cdarchive.mjs   the command line
selftest/           gates whose answers are known before the code runs
session/            scripted play against the LOCAL replica, to generate real data
fixtures/           real archived hands, copied verbatim, used by the gates
  one-hand.json       three seats, a re-raise, an uncalled bet, a showdown, one folder
  uncalled-bet.json   a bet nobody called, so the chips wagered exceed the pot
  departed-blind.json a big blind that left before the action and got half of it back
```

## Use it

```bash
# 1. fetch (the only networked step; local replica by default).
#    --out is required and has no default inside the repo: a bundle is megabytes of
#    generated JSON and does not belong in a tracked tree.
node tools/archive/fetch/fetch-archive.mjs --out /tmp/cleardeck-bundle.json

# 2. everything else is offline
node tools/archive/bin/cdarchive.mjs verify    /tmp/cleardeck-bundle.json
node tools/archive/bin/cdarchive.mjs hand      /tmp/cleardeck-bundle.json --id 181 --trace --ev
node tools/archive/bin/cdarchive.mjs stats     /tmp/cleardeck-bundle.json
node tools/archive/bin/cdarchive.mjs ev        /tmp/cleardeck-bundle.json
node tools/archive/bin/cdarchive.mjs collusion /tmp/cleardeck-bundle.json
```

Every command takes `--table <principal>`, `--from-hand <id>` and `--to-hand <id>`
to narrow the window. `verify` exits non-zero if any card disagrees with the seed
or any hand's money fails to add up.

## The gates

```bash
node tools/archive/selftest/run.mjs      # 39 cases, no replica, no network
node tools/archive/selftest/power.mjs <bundle>   # how many hands a collusion signal needs
```

`run.mjs` is the gate that holds every claim in `docs/ARCHIVE.md`. It replays the
2,000 committed golden vectors through this tool's own shuffle, reproduces
`SHUFFLE-SPEC` section 5 card for card, counts all 2,598,960 five-card hands by
category against their textbook totals, moves one pot credit from the winner to a
loser and checks that the tool notices even though every sum still balances, and —
for the collusion signals — measures the false-positive rate on populations that
are honest by construction.

There is one more check that needs a replica, and it is the one that matters most
for the folded cards, because nothing in the archive can check them:

```bash
node tools/archive/session/verify-live.mjs --table table_1 --players 1,2 --hands 45
```

It reads every seat's own cards WHILE the hand is live, then reconstructs from the
archive and compares. 45 hands, 180 hole cards, 156 of them cards no record
publishes, zero disagreements.

## Generating real data on the local replica

```bash
./scripts/dev.sh local-up
node tools/archive/session/play-session.mjs --table table_2 --players 1,2,3 --hands 1200 --mode honest  --seed 77
node tools/archive/session/play-session.mjs --table table_3 --players 4,5,6 --hands 1200 --mode collude --pair 4,5 --seed 88
```

Both sessions play real hands through the real table canister and the real
settlement path; the scripted players only choose actions, exactly as a browser
would. `--mode collude` makes one of the pair fold the river to the other whatever
it is holding, which is a chip dump and is what the detector is pointed at.

**LOCAL ONLY.** `fetch/fetch-archive.mjs` reads the mainnet id list out of
`.icp/data/mappings/ic.ids.json` and refuses any argument that names one, and
refuses the `ic` environment outright. `session/` inherits the same guard.
