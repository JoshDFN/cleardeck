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
SELFTEST=0
while [ $# -gt 0 ]; do
  case "$1" in
    --network)  NETWORK="$2"; shift 2 ;;
    --identity) IDENTITY="$2"; shift 2 ;;
    --selftest) SELFTEST=1; shift ;;
    -h|--help)  sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

# ---------------------------------------------------------------------------
# READING ONE FIELD OUT OF A CANDID REPLY, AND WHY IT HAS A SELF-TEST
# ---------------------------------------------------------------------------
#
# THIS GUARD COMPARED ONE FIELD OUT OF EIGHT AND SAID "matches" (docs/DEFECTS.md
# L-04, found wave 14). The extractor was:
#
#     grep -E "^[[:space:]]*$f = " | head -1 | grep -oE '[0-9_]+' | head -1 | tr -d '_'
#
# and `[0-9_]+` matches THE UNDERSCORE INSIDE THE FIELD NAME. On the line
#
#     "      small_blind = 5_000_000 : nat64"
#
# the first match of `[0-9_]+` is the `_` in `small_blind`; `head -1` takes it,
# `tr -d '_'` empties it, and the caller's `[ -n "$got" ] || continue` then
# SKIPPED THE FIELD. Seven of the eight names in TableConfig contain an
# underscore, so the only field this script ever compared was `ante`.
#
# Measured before the fix: table_2's contract was moved to
# `action_timeout_secs = 60` while icp.yaml declares 45, and the script printed
#
#     ✓ table_2 (4fbx2-kt777-77775-aaabq-cai) matches icp.yaml
#
# This is the guard written in wave 13 to stop btc_table_1 running at one tenth of
# its declared stakes for its whole life, and it would not have caught that
# either. `.github/workflows/deployed-drift.yml` has been running it against
# MAINNET daily and reporting green.
#
# So: one helper, used by both comparisons, and a `--selftest` that proves it can
# read a value AND that a planted mismatch is convicted. An extractor with no
# self-test is how the first one got here.
field_value() {
  local reply="$1" field="$2"
  printf '%s' "$reply" \
    | tr ';' '\n' \
    | grep -E "^[[:space:]]*$field = " \
    | head -1 \
    | sed -E 's/^[^=]*=[[:space:]]*([0-9_]+).*$/\1/' \
    | tr -d '_'
}

if [ "$SELFTEST" = "1" ]; then
  echo "self-test: the field extractor"
  st_fail=0
  sample=$'  record {\n      small_blind = 5_000_000 : nat64;\n      time_bank_secs = 30 : nat64;\n      action_timeout_secs = 45 : nat64;\n      ante = 0 : nat64;\n      max_players = 6 : nat8;\n      min_buy_in = 1_000_000_000 : nat64;\n      max_buy_in = 5_000_000_000 : nat64;\n      big_blind = 10_000_000 : nat64;\n  }'
  check_field() {
    local f="$1" want="$2" got
    got="$(field_value "$sample" "$f")"
    if [ "$got" = "$want" ]; then
      echo "  ok    $f -> $got"
    else
      echo "  FAIL  $f -> '$got', expected '$want'"
      st_fail=1
    fi
  }
  # Every field name that carries an underscore is here on purpose: those are the
  # seven the old extractor silently skipped.
  check_field small_blind 5000000
  check_field big_blind 10000000
  check_field min_buy_in 1000000000
  check_field max_buy_in 5000000000
  check_field max_players 6
  check_field action_timeout_secs 45
  check_field time_bank_secs 30
  check_field ante 0
  # And it must report NOTHING for a field that is absent, so "could not read" is
  # distinguishable from "read a zero".
  if [ -z "$(field_value "$sample" "rake_bps")" ]; then
    echo "  ok    an absent field reads as empty, not as 0"
  else
    echo "  FAIL  an absent field did not read as empty"
    st_fail=1
  fi
  # The conviction itself: a planted disagreement must not compare equal.
  if [ "$(field_value "$sample" small_blind)" = "1000000" ]; then
    echo "  FAIL  5_000_000 compared equal to the declared 1_000_000"
    st_fail=1
  else
    echo "  ok    a 5x drift in small_blind is not equal to the declaration"
  fi
  if [ "$st_fail" -ne 0 ]; then
    echo "SELF-TEST FAILED: this script cannot read the values it claims to compare." >&2
    exit 1
  fi
  echo "self-test passed"
  echo
fi

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
    got="$(field_value "$live_raw" "$f")"
    [ -n "$got" ] || { echo "  ! $name: could not read '$f' from the live reply"; fail=1; continue; }
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

# ---------------------------------------------------------------------------
# THE LOBBY REGISTRY AGAINST THE CONTRACTS IT ADVERTISES (docs/DEFECTS.md L-04)
# ---------------------------------------------------------------------------
#
# The check above answers "does the canister run what icp.yaml declared". This one
# answers the question a PLAYER asks, which is a different one: does the price on
# the list match the price the contract charges. It did not. The lobby registered
# small_blind/big_blind/min_buy_in/max_buy_in when each table was created and
# never looked again, so ids 2 and 3 advertised 0.01/0.02 while their contracts
# charged 0.05/0.10 and 0.10/0.20 -- four wrong figures per row, on the one screen
# whose entire argument is "you can check this yourself". The client already reads
# the true figure from each table contract and strikes the lobby's through, which
# is how it was caught and is not a substitute for the registry being right.
#
# Both halves of a lobby row are checked: the CONFIG record, and the NAME, which
# must not restate a price at all (wave 13 removed every price-bearing name for
# the reason a name that restates another canister's number can only ever drift).
#
# Read-only: two queries per table, no identity.
lobby_id="$(python3 -c "import json;print(json.load(open('$MAP')).get('lobby',''))")"
if [ -z "$lobby_id" ]; then
  echo
  echo "  lobby: not in $MAP, skipping the registry check"
else
  echo
  echo "lobby registry vs the table contracts it advertises (L-04)"
  registry="$(icp canister call "$lobby_id" get_tables '()' -e "$NETWORK" "${ID_ARG[@]+"${ID_ARG[@]}"}" --query 2>/dev/null || true)"
  if [ -z "$registry" ]; then
    echo "  ✗ lobby ($lobby_id): could not read get_tables()"
    fail=1
  else
    # The registry text goes in through the ENVIRONMENT, not a pipe: `python3 -`
    # with a heredoc reads its PROGRAM from stdin, so a pipe into it is silently
    # discarded and every row parses as absent. That is an instrument that
    # measures nothing and passes, which is the one failure this repository has
    # already had nine times.
    rows="$(CLEARDECK_REGISTRY="$registry" python3 - <<'PY'
import os, re
raw = os.environ["CLEARDECK_REGISTRY"]
# One record per registered table. Split on the id marker and pull the fields we
# compare; the reply is Candid text, so this is a parse of a known shape, not of
# arbitrary input.
for m in re.finditer(r"id = (\d+) : nat64;(.*?)\n    \};", raw, re.S):
    tid, body = m.group(1), m.group(2)
    cid = re.search(r'canister_id = opt principal "([^"]+)"', body)
    name = re.search(r'name = "([^"]*)"', body)
    fields = {}
    for f in ("small_blind", "big_blind", "min_buy_in", "max_buy_in", "max_players",
              "ante", "action_timeout_secs", "time_bank_secs"):
        v = re.search(r"\b%s = ([0-9_]+)" % f, body)
        if v:
            fields[f] = v.group(1).replace("_", "")
    if not cid:
        print(f"{tid}|NONE|{name.group(1) if name else ''}|")
        continue
    print("%s|%s|%s|%s" % (tid, cid.group(1), name.group(1) if name else "",
                           ",".join(f"{k}={v}" for k, v in fields.items())))
PY
)"
    lobby_rows=0
    while IFS='|' read -r tid tcid tname tfields; do
      [ -n "$tid" ] || continue
      lobby_rows=$((lobby_rows + 1))
      if [ "$tcid" = "NONE" ]; then
        echo "  ✗ lobby row $tid (\"$tname\") has no canister id: nothing can check what it advertises"
        fail=1
        continue
      fi
      contract="$(icp canister call "$tcid" get_table_view '()' -e "$NETWORK" "${ID_ARG[@]+"${ID_ARG[@]}"}" --query 2>/dev/null || true)"
      if [ -z "$contract" ]; then
        echo "  ✗ lobby row $tid ($tcid): the contract did not answer get_table_view()"
        fail=1
        continue
      fi
      bad=""
      IFS=',' read -ra pairs <<< "$tfields"
      for pair in "${pairs[@]}"; do
        f="${pair%%=*}"; want="${pair#*=}"
        got="$(field_value "$contract" "$f")"
        [ -n "$got" ] || { bad="$bad\n      $f: the contract's reply has no such field"; continue; }
        [ "$got" = "$want" ] || bad="$bad\n      $f: lobby advertises $want, the contract charges $got"
      done
      # A NAME MUST NOT QUOTE A PRICE. Three names carried "0.01/0.02" while two of
      # the three contracts charged something else; the durable fix was to take the
      # price out of the name, so a name with digits and a slash in it is a
      # regression whatever the digits say.
      if printf '%s' "$tname" | grep -qE '[0-9]+(\.[0-9]+)?/[0-9]+'; then
        bad="$bad\n      name \"$tname\" quotes a price. A registered name restates a figure owned by the table contract and can only drift; the stakes column already reads the contract."
      fi
      if [ -n "$bad" ]; then
        echo "  ✗ lobby row $tid (\"$tname\") -> $tcid"
        printf "%b\n" "$bad"
        fail=1
      else
        echo "  ✓ lobby row $tid (\"$tname\") matches $tcid"
      fi
    done <<< "$rows"
    if [ "$lobby_rows" -eq 0 ]; then
      echo "  ✗ lobby ($lobby_id) registered NO tables; the list a player sees is empty"
      fail=1
    else
      echo "  $lobby_rows lobby row(s) checked against their contracts"
    fi
  fi
fi

echo
if [ "$fail" -ne 0 ]; then
  echo "DECLARED CONFIG DOES NOT MATCH WHAT IS DEPLOYED."
  echo "icp.yaml's init_args only apply at first install; an upgrade preserves the"
  echo "original values. Either fix the running canister with admin_update_config, or"
  echo "correct icp.yaml so it stops describing a table that does not exist."
  echo
  echo "If the LOBBY rows are the ones that disagree, the fix is ONE admin-only lobby"
  echo "update -- refresh_all_table_configs(), which COPIES each config out of its own"
  echo "table contract rather than taking one as an argument. The exact invocation is in"
  echo "docs/DEFECTS.md L-04; it is deliberately not printed here, because this script"
  echo "is proved read-only by a guard that greps it for canister calls that are not"
  echo "queries, and a guard that cannot tell a suggestion from an instruction is a"
  echo "guard somebody eventually switches off."
  exit 1
fi
echo "all $checked table config(s) match icp.yaml, and every lobby row matches its contract"
