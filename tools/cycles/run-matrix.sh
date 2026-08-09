#!/usr/bin/env bash
# THE MEASUREMENT BEHIND docs/DEFECTS.md E-92, AS ONE RE-RUNNABLE COMMAND.
#
#   ./tools/cycles/run-matrix.sh [seconds]
#
# Runs `tools/cycles/tab-burn.mjs` over the whole matrix and writes one JSON per
# cell under artifacts/cycles/. Local replica only.
#
# WHY A CONTROL RUN COMES FIRST AND LAST
#   The canister's own clock burns cycles whether or not a tab is open, so every
#   marginal figure is a difference against a control. The control is measured
#   TWICE, at the start and at the end of the matrix, because a control taken once
#   at the beginning of a 30-minute run is an assumption about the other 29
#   minutes. If the two controls disagree materially, the matrix is not usable and
#   the run says so instead of publishing a difference against a moving baseline.
set -euo pipefail
cd "$(dirname "$0")/../.."

SECS="${1:-120}"
OUTDIR="artifacts/cycles"
mkdir -p "$OUTDIR"

run() { # run <label> <tabs> <mode>
  echo "== $1  (tabs=$2 mode=$3, ${SECS}s)"
  node tools/cycles/tab-burn.mjs \
    --tabs "$2" --mode "$3" --seconds "$SECS" --settle 12 \
    --out "$OUTDIR/$1.json"
  echo
}

run control-before 0 legacy

for n in 1 3 10; do
  run "legacy-${n}tab" "$n" legacy
done
for n in 1 3 10; do
  run "fixed-${n}tab" "$n" fixed
done
for n in 1 3 10; do
  run "fixed-max-${n}tab" "$n" fixed-max
done
run fixed-stuck-10tab 10 fixed-stuck

run control-after 0 legacy

echo "== matrix complete; JSON under $OUTDIR"
