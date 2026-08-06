#!/usr/bin/env bash
# Does the config a canister is ACTUALLY running match the config icp.yaml declares?
#
# WHY THIS EXISTS
#   `icp deploy --mode upgrade` does not re-run init args. It cannot: init args are
#   install-time input, and an upgrade deliberately preserves state. So the moment a
#   canister is installed, its `init_args` block in icp.yaml stops describing it and
#   starts describing an intention.
#
#   That is not hypothetical. btc_table_1 ran on blinds of 10/200 sats and a buy-in
#   range of 1,000-20,000 for its entire life, while icp.yaml declared 100/200 and
#   10,000-100,000 -- exactly ten times larger, across every field. Nine upgrades,
#   four blind audits and a full CI suite never noticed, because nothing anywhere
#   compared the declaration against the deployment. It was found by a player-facing
#   strikethrough in the lobby.
#
#   The lesson generalises: any value that is declared in one place and stored in
#   another will drift, and it will drift silently unless something compares them.
#
# WHAT IT DOES
#   Reads each table's live TableConfig off the canister and diffs it against the
#   init_args in icp.yaml, field by field. Exits non-zero on any disagreement.
#
# USAGE
#   ./scripts/check-deployed-config.sh --network local   # against the local replica
#   ./scripts/check-deployed-config.sh --network ic --identity <controller>
#
#   It only ever READS. It never deploys, never writes and never moves money.
set -euo pipefail

NETWORK="local"
IDENTITY=""
while [ $# -gt 0 ]; do
  case "$1" in
    --network)  NETWORK="$2"; shift 2 ;;
    --identity) IDENTITY="$2"; shift 2 ;;
    -h|--help)  sed -n '2,30p' "$0"; exit 0 ;;
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
[ -f "$MAP" ] && : || { echo "no id mapping at $MAP -- is the stack deployed?" >&2; exit 1; }

FIELDS="small_blind big_blind min_buy_in max_buy_in max_players action_timeout_secs ante time_bank_secs"
fail=0
checked=0

for name in table_1 table_2 table_3 btc_table_1; do
  cid="$(python3 -c "import json,sys;print(json.load(open('$MAP')).get('$name',''))")"
  if [ -z "$cid" ]; then
    echo "  $name: not in $MAP, skipping"
    continue
  fi

  # what icp.yaml DECLARES for this canister
  declared="$(python3 - "$name" <<'PY'
import re, sys
name = sys.argv[1]
y = open("icp.yaml").read()
m = re.search(r"- name: %s\b.*?init_args:\s*\n\s*value: '([^']*)'" % re.escape(name), y, re.S)
if not m:
    sys.exit(0)
for f in "small_blind big_blind min_buy_in max_buy_in max_players action_timeout_secs ante time_bank_secs".split():
    v = re.search(r"\b%s\s*=\s*([0-9_]+)" % f, m.group(1))
    if v:
        print(f, v.group(1).replace("_", ""))
PY
)"
  [ -n "$declared" ] || { echo "  $name: no init_args in icp.yaml, skipping"; continue; }

  # what the canister is RUNNING
  live_raw="$(icp canister call "$cid" get_table_view '()' -e "$NETWORK" "${ID_ARG[@]+"${ID_ARG[@]}"}" --query 2>/dev/null || true)"
  [ -n "$live_raw" ] || { echo "  $name ($cid): could not read live config"; fail=1; continue; }

  checked=$((checked + 1))
  bad=""
  while read -r f want; do
    [ -n "$f" ] || continue
    got="$(printf '%s' "$live_raw" | tr ';' '\n' | grep -E "^[[:space:]]*$f = " | head -1 | grep -oE '[0-9_]+' | head -1 | tr -d '_')"
    [ -n "$got" ] || continue
    [ "$got" = "$want" ] || bad="$bad\n      $f: declared $want, running $got"
  done <<< "$declared"

  if [ -n "$bad" ]; then
    echo "  ✗ $name ($cid)"
    printf "%b\n" "$bad"
    fail=1
  else
    echo "  ✓ $name ($cid) matches icp.yaml"
  fi
done

echo
if [ "$fail" -ne 0 ]; then
  echo "DECLARED CONFIG DOES NOT MATCH WHAT IS DEPLOYED."
  echo "icp.yaml's init_args only apply at first install; an upgrade preserves the"
  echo "original values. Either fix the running canister with admin_update_config, or"
  echo "correct icp.yaml so it stops describing a table that does not exist."
  exit 1
fi
echo "all $checked table config(s) match icp.yaml"
