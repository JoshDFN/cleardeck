#!/usr/bin/env bash
# Does the deployment still match this source tree?
#
# WHY THIS EXISTS
#   This repository's central claim is that the code running in the canisters is
#   the code in the repository. That claim decays silently: nothing about a
#   canister changes when main moves on, so the moment main gains a commit the
#   deployment starts being a description of the past. The project has been bitten
#   by exactly this four times (see docs/DECLARED-VS-STORED.md), most sharply when
#   a table ran at one tenth its declared stakes for its entire life because
#   `icp deploy --mode upgrade` does not re-run init args.
#
#   Every one of those was found by a person who went looking. This runs on a
#   schedule instead, so the repository reports drift rather than waiting to be
#   asked.
#
# WHAT IT READS, AND ONLY READS
#   * `icp canister metadata <id> <section>` for the PUBLIC sections every module
#     publishes -- git:revision, git:dirty, candid:service. Public metadata needs
#     no controller rights and no identity at all.
#   * `icp canister call --query` for the live table config, via
#     scripts/check-deployed-config.sh.
#
#   IT NEVER DEPLOYS, NEVER INSTALLS, NEVER WRITES AND NEVER MOVES MONEY. There
#   is no code path here that calls an update method. The only `icp` subcommands
#   used are `metadata` and `call --query`.
#
# USAGE
#   ./scripts/check-deployed.sh --network local
#   ./scripts/check-deployed.sh --network ic [--expect-revision <sha>]
set -euo pipefail

NETWORK="local"
IDENTITY=""
EXPECT_REV=""
REQUIRE_CLEAN=0
while [ $# -gt 0 ]; do
  case "$1" in
    --network)         NETWORK="$2"; shift 2 ;;
    --identity)        IDENTITY="$2"; shift 2 ;;
    --expect-revision) EXPECT_REV="$2"; shift 2 ;;
    --require-clean)   REQUIRE_CLEAN=1; shift ;;
    -h|--help)         sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

cd "$(dirname "$0")/.."
ID_ARG=()
[ -n "$IDENTITY" ] && ID_ARG=(--identity "$IDENTITY")

if [ "$NETWORK" = "ic" ]; then
  MAP=".icp/data/mappings/ic.ids.json"
else
  MAP=".icp/cache/mappings/local.ids.json"
fi
[ -f "$MAP" ] || { echo "no id mapping at $MAP" >&2; exit 1; }

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
fail=0

# canister name -> the crate whose .did it must publish
did_for() {
  case "$1" in
    lobby)   echo src/lobby_canister/lobby_canister.did ;;
    history) echo src/history_canister/history_canister.did ;;
    table_1|table_2|table_3|btc_table_1) echo src/table_canister/table_canister.did ;;
    *) echo "" ;;
  esac
}

cid_of() {
  python3 -c "import json;print(json.load(open('$MAP')).get('$1',''))"
}

meta() { # meta <cid> <section>
  icp canister metadata "$1" "$2" -e "$NETWORK" "${ID_ARG[@]+"${ID_ARG[@]}"}" 2>/dev/null || true
}

NAMES="lobby history table_1 table_2 table_3 btc_table_1"

echo "== what revision each canister says it is (public git:revision metadata)"
revs=""
seen=0
for n in $NAMES; do
  cid="$(cid_of "$n")"
  [ -n "$cid" ] || { printf '  %-12s not in %s, skipping\n' "$n" "$MAP"; continue; }
  rev="$(meta "$cid" git:revision | head -1 | tr -d '[:space:]')"
  dirty="$(meta "$cid" git:dirty | head -1 | tr -d '[:space:]')"
  if [ -z "$rev" ]; then
    printf '  ✗ %-12s %s  could not read git:revision\n' "$n" "$cid"
    fail=1
    continue
  fi
  seen=$((seen + 1))
  revs="$revs$rev\n"
  printf '  %-12s %s  rev=%s dirty=%s\n' "$n" "$cid" "${rev:0:12}" "$dirty"
  if [ "$dirty" = "dirty" ] && [ "$REQUIRE_CLEAN" -eq 1 ]; then
    echo "      ✗ deployed from a DIRTY tree: no commit describes these bytes"
    fail=1
  fi
  if [ -n "$EXPECT_REV" ] && [ "$rev" != "$EXPECT_REV" ]; then
    echo "      ✗ expected revision $EXPECT_REV"
    fail=1
  fi
done

[ "$seen" -gt 0 ] || { echo "::error::read no canisters at all -- this check proved nothing"; exit 1; }

distinct="$(printf "%b" "$revs" | grep -c . || true)"
uniq_n="$(printf "%b" "$revs" | sort -u | grep -c . || true)"
if [ "$uniq_n" -gt 1 ]; then
  echo "  ✗ the fleet is SPLIT across $uniq_n revisions -- not one deployment:"
  printf "%b" "$revs" | sort | uniq -c | sed 's/^/      /'
  fail=1
else
  echo "  ✓ all $distinct canisters report the same revision"
fi

echo
echo "== the interface each canister PUBLISHES vs the committed .did"
# This is the live half of docs/DECLARED-VS-STORED.md row D1. `icp build` embeds
# the .did as public candid:service metadata, so a canister that has not been
# redeployed since the .did was corrected still publishes the old shape -- and
# every client generated from it believes the old shape.
checked=0
for n in $NAMES; do
  cid="$(cid_of "$n")"; [ -n "$cid" ] || continue
  did="$(did_for "$n")"; [ -n "$did" ] || continue
  meta "$cid" candid:service > "$TMP/$n.did"
  if [ ! -s "$TMP/$n.did" ]; then
    printf '  ✗ %-12s published no candid:service metadata\n' "$n"
    fail=1
    continue
  fi
  checked=$((checked + 1))
  if python3 scripts/candid_compare.py "$did" "$TMP/$n.did" \
        --label-a SOURCE --label-b LIVE > "$TMP/$n.out" 2>&1; then
    printf '  ✓ %-12s publishes exactly %s\n' "$n" "$did"
  else
    printf '  ✗ %-12s publishes an interface that is NOT %s\n' "$n" "$did"
    sed 's/^/      /' "$TMP/$n.out" | grep -vE '^\s*$' | head -20
    fail=1
  fi
done
[ "$checked" -gt 0 ] || { echo "::error::compared no interfaces -- this check proved nothing"; exit 1; }

echo
echo "== the config each table is RUNNING vs the init_args icp.yaml declares"
if ./scripts/check-deployed-config.sh --network "$NETWORK" ${IDENTITY:+--identity "$IDENTITY"}; then
  :
else
  fail=1
fi

echo
if [ "$fail" -ne 0 ]; then
  echo "THE DEPLOYMENT DOES NOT MATCH THIS SOURCE TREE."
  echo "Nothing here is fixed by editing the source: the canisters must be"
  echo "redeployed, or the source corrected to describe what is actually running."
  exit 1
fi
echo "deployment matches source on network '$NETWORK'"
