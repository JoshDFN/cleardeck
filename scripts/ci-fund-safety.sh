#!/usr/bin/env bash
#
# scripts/ci-fund-safety.sh <fast|deep> [--list] -- THE FUND-SAFETY GATE, IN CI.
#
# docs/DEFECTS.md H-23. Until this file existed, `.github/workflows/ci.yml` ran
# `cargo test --locked --workspace`, and both `tests/money_safety` and
# `tests/settlement` carry a bare `[workspace]` so that command does not see them.
# The money invariants, the settlement oracle, the fuzzer, the custody gate and
# the solvency gate had therefore never run in CI -- only on somebody's laptop,
# when they remembered. Every fund-safety sentence in docs/SECURITY-FINDINGS.md
# and docs/DEFECTS.md rests on those harnesses.
#
# THIS IS THE JOB BODY, NOT A DESCRIPTION OF IT. The workflow files call this
# script and nothing else, so the thing a developer can run on a laptop and the
# thing that gates a merge are the same bytes. A CI job written inline in YAML is
# a job nobody can reproduce, and an unreproducible gate is how a pipeline comes
# to be green for reasons no one understands.
#
# WHAT IT RUNS is scripts/test-suites.list, filtered to one tier. Adding a suite
# to CI is adding a row there; scripts/check-suite-wiring.sh fails the build if a
# test target exists with no row.
#
#   fast   the REQUIRED check on every pull request
#   deep   the scheduled run
#
# LOCAL REPLICA ONLY. PocketIC starts an in-process replica. Nothing here can
# reach mainnet: no canister id, no -e ic, no wallet. The only network access is
# two pinned, checksummed downloads (the PocketIC server and the real ICP ledger
# module), both of which are cached.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INVENTORY="$REPO_ROOT/scripts/test-suites.list"

G=$'\033[32m'; Y=$'\033[33m'; E=$'\033[31m'; B=$'\033[1m'; R=$'\033[0m'
step() { printf '\n%s==> %s%s\n' "$B" "$*" "$R"; }
info() { printf '    %s\n' "$*"; }
ok()   { printf '    %sok%s  %s\n' "$G" "$R" "$*"; }
die()  { printf '    %sERROR%s %s\n' "$E" "$R" "$*" >&2; exit 1; }

TIER="${1:-}"
case "$TIER" in
  fast|deep) ;;
  *) printf 'usage: %s <fast|deep> [--list]\n' "$0" >&2; exit 2 ;;
esac
LIST_ONLY="${2:-}"

rows() {
  grep -v '^[[:space:]]*#' "$INVENTORY" | grep -v '^[[:space:]]*$' \
    | awk -F'|' -v t="$TIER" '$3 == t'
}

# ---------------------------------------------------------------------------
# 0. what this run will do, before it does any of it
# ---------------------------------------------------------------------------
step "tier: $TIER"
n_rows="$(rows | wc -l | tr -d ' ')"
[ "$n_rows" -gt 0 ] || die "no rows in scripts/test-suites.list for tier '$TIER'.
    A tier that selects nothing is a job that passes without testing anything,
    which is the exact failure this file exists to end."
info "$n_rows suite invocation(s) from scripts/test-suites.list"
while IFS='|' read -r crate target tier threads filter env notetext; do
  printf '      %-22s %-28s %s\n' "$crate" "$target" \
    "$( [ "$filter" = '-' ] && echo '' || echo "-- $filter" )"
done < <(rows)
[ "$LIST_ONLY" = "--list" ] && exit 0

# ---------------------------------------------------------------------------
# 1. the replica
# ---------------------------------------------------------------------------
step "PocketIC server"
POCKET_IC_BIN="$("$REPO_ROOT/scripts/pocket-ic.sh")" || die "could not resolve a PocketIC server"
export POCKET_IC_BIN
ok "$POCKET_IC_BIN"

# ---------------------------------------------------------------------------
# 2. the ledger
# ---------------------------------------------------------------------------
#
# THE PIN IS READ OUT OF THE HARNESS, NOT RESTATED HERE. tests/money_safety's own
# wasms.rs carries the URL and the sha256, and it hash-checks whatever it is given.
# Writing the hash into this file too would make two lists that can drift, and a
# CI job that fetches a DIFFERENT ledger than the harness pins would invalidate
# every M2 (LEDGER REALITY) result while staying green.
step "the real ICP ledger module"
WASMS_RS="$REPO_ROOT/tests/money_safety/src/wasms.rs"
LEDGER_URL="$(grep -oE 'https://download\.dfinity\.systems/[^"]*ledger-canister\.wasm\.gz' "$WASMS_RS" | head -1)"
LEDGER_SHA="$(grep -A2 'ICP_LEDGER_SHA256' "$WASMS_RS" | grep -oE '"[0-9a-f]{64}"' | head -1 | tr -d '"')"
[ -n "$LEDGER_URL" ] && [ -n "$LEDGER_SHA" ] \
  || die "could not read the ledger pin out of tests/money_safety/src/wasms.rs.
    That file owns the pin; if its shape changed, fix this reader rather than
    hardcoding a second copy of the hash."
LEDGER_CACHE="${CLEARDECK_CACHE_DIR:-$REPO_ROOT/target}/money-safety/ledger-canister.wasm.gz"
mkdir -p "$(dirname "$LEDGER_CACHE")"
sha_of() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  else shasum -a 256 "$1" | cut -d' ' -f1; fi
}
if [ -f "$LEDGER_CACHE" ] && [ "$(sha_of "$LEDGER_CACHE")" = "$LEDGER_SHA" ]; then
  ok "cached, sha256 $LEDGER_SHA"
else
  info "fetching $LEDGER_URL"
  curl -sSL --fail --retry 3 --retry-delay 2 --max-time 300 -o "$LEDGER_CACHE.tmp" "$LEDGER_URL" \
    || die "could not fetch the ICP ledger module"
  got="$(sha_of "$LEDGER_CACHE.tmp")"
  [ "$got" = "$LEDGER_SHA" ] || { rm -f "$LEDGER_CACHE.tmp"; die "ledger sha256 $got != pinned $LEDGER_SHA"; }
  mv -f "$LEDGER_CACHE.tmp" "$LEDGER_CACHE"
  ok "fetched, sha256 $got"
fi
export ICP_LEDGER_WASM="$LEDGER_CACHE"

# ---------------------------------------------------------------------------
# 3. the module under test
# ---------------------------------------------------------------------------
#
# Built here, from the checked-out source, so a CI run cannot pass against a
# cached artifact from a different commit. The harness records the sha256 of what
# the replica is actually running and asserts it is this file.
step "build table_canister.wasm from this checkout"
( cd "$REPO_ROOT" && cargo build --locked -p table_canister --target wasm32-unknown-unknown --release ) \
  || die "the table canister did not build"
TABLE_WASM="$REPO_ROOT/target/wasm32-unknown-unknown/release/table_canister.wasm"
[ -s "$TABLE_WASM" ] || die "$TABLE_WASM is missing or empty"
sz=$(wc -c < "$TABLE_WASM" | tr -d ' ')
[ "$sz" -gt 500000 ] || die "$TABLE_WASM is only $sz bytes, that is not the canister"
export CLEARDECK_TABLE_WASM="$TABLE_WASM"
ok "$(sha_of "$TABLE_WASM")  ($sz bytes)"

# ---------------------------------------------------------------------------
# 4. the suites
# ---------------------------------------------------------------------------
#
# EVERY ROW REPORTS ITS OWN WALL TIME, and a run that executed no test binary is
# a failure rather than a pass. `cargo test` exits 0 when a filter matches
# nothing, so a renamed filter would otherwise turn a gate into a no-op that
# reads green -- the exact shape of every defect this project keeps finding.
failed=()
declare -a TIMES=()
total_start=$(date +%s)

for_each_row() {
  local crate target tier threads filter env notetext
  while IFS='|' read -r crate target tier threads filter env notetext; do
    # `${a[@]+"${a[@]}"}` rather than `"${a[@]}"`: under `set -u`, expanding an
    # empty array is an error in bash 3.2, which is still /bin/bash on macOS.
    #
    # `(lib)` is the pseudo-target for unit tests compiled into the crate itself.
    # `--test X` and `--lib` are DIFFERENT cargo targets, and every runner in this
    # repo names the former, which is how a `#[cfg(test)]` module inside
    # tests/money_safety/src came to be run by nothing at all.
    local -a cmd
    if [ "$target" = '(lib)' ]; then
      cmd=(cargo test --locked --lib)
    else
      cmd=(cargo test --locked --test "$target")
    fi
    local -a post=()
    [ "$threads" != '-' ] && post+=(--test-threads="$threads")
    [ "$filter" != '-' ] && post=("$filter" ${post[@]+"${post[@]}"})
    [ "${#post[@]}" -gt 0 ] && cmd+=(-- "${post[@]}")

    local -a envs=()
    if [ "$env" != '-' ]; then
      # A row's env is applied to THIS row only. `env -u` first, so a variable
      # left in the ambient environment cannot silently steer a run that is
      # meant to be at its own defaults (docs/DEFECTS.md H-28: every caller
      # passed MONEY_FUZZ_SEEDS, so the default arm was executed by nothing).
      read -r -a envs <<<"$env"
    fi

    local label
    if [ "$target" = '(lib)' ]; then label="$crate --lib"; else label="$crate --test $target"; fi
    [ "$filter" != '-' ] && label="$label -- $filter"
    [ "$env" != '-' ] && label="$label  [$env]"
    step "$label"
    info "$notetext"

    local s e rc log
    # The index is in the filename because one target can legitimately appear
    # TWICE in a tier with different settings -- the deep tier runs `fuzz` at its
    # own defaults and again at 9 seeds x 600 -- and two rows sharing a log file
    # would silently discard the first one's evidence.
    row_index=$((${row_index:-0} + 1))
    log="$REPO_ROOT/target/ci-fund-safety/$(printf '%02d' "$row_index")-${crate//\//_}-${target//[^a-z_0-9]/}.log"
    mkdir -p "$(dirname "$log")"
    s=$(date +%s)
    (
      cd "$REPO_ROOT/$crate" || exit 1
      # env -u MONEY_FUZZ_* is deliberate and unconditional: the deep tier has a
      # row that must run the fuzzer with NOTHING in the environment, and it
      # shares this loop with a row that sets those variables.
      exec env -u MONEY_FUZZ_SEEDS -u MONEY_FUZZ_STEPS -u MONEY_FUZZ_SHRINK -u MONEY_FUZZ_REPORT \
               -u SETTLEMENT_RANDOM_HANDS -u SETTLEMENT_RECORD_ONLY -u SETTLEMENT_EXPECT_WRONG_SEAT \
               ${envs[@]+"${envs[@]}"} "${cmd[@]}"
    ) 2>&1 | tee "$log"
    rc=${PIPESTATUS[0]}
    e=$(date +%s)

    # THE RUN MUST HAVE RUN SOMETHING.
    if ! grep -qE '^test result: ' "$log"; then
      printf '    %sNO TEST BINARY REPORTED%s -- this row executed nothing\n' "$E" "$R" >&2
      rc=1
    elif grep -qE '^test result: ok\. 0 passed' "$log" && [ "$filter" != '-' ]; then
      printf '    %sFILTER MATCHED NOTHING%s -- "%s" selects no test in %s\n' "$E" "$R" "$filter" "$target" >&2
      rc=1
    fi

    local passed
    passed="$(grep -oE '^test result: ok\. [0-9]+ passed' "$log" | awk '{s += $4} END {print s + 0}')"
    TIMES+=("$(printf '%-46s %5ss  %s passed' "$label" "$((e - s))" "$passed")")
    if [ "$rc" != 0 ]; then
      printf '    %sFAILED%s after %ss\n' "$E" "$R" "$((e - s))" >&2
      failed+=("$label")
    else
      ok "$passed test(s) in $((e - s))s"
    fi
  done < <(rows)
}
for_each_row

total_end=$(date +%s)

step "runtime, row by row"
printf '%s\n' "${TIMES[@]}" | sed 's/^/    /'
printf '    %s\n' "----------------------------------------------------------------"
printf '    %-46s %5ss\n' "TOTAL ($TIER tier)" "$((total_end - total_start))"

step "result"
if [ "${#failed[@]}" -eq 0 ]; then
  printf '    %sthe %s fund-safety tier is green%s\n' "$G" "$TIER" "$R"
  exit 0
fi
printf '    %sFAILED:%s\n' "$E" "$R" >&2
printf '      %s\n' "${failed[@]}" >&2
exit 1
