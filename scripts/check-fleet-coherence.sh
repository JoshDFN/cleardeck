#!/usr/bin/env bash
# Do canisters built from the SAME package actually run the same code?
#
# WHY THIS EXISTS
#   table_1, table_2, table_3 and btc_table_1 are four instances of one package,
#   `table_canister`. They differ only in init_args, and init args are install-time
#   arguments -- they are not part of the module. So all four MUST report the same
#   module hash. On 2026-08-09 they reported three:
#
#       table_1      4511ab187cff8b90...c608
#       table_2      9c0ed3a138a3753d...537c
#       table_3      511c9d0e6d508d62...6ba4
#       btc_table_1  9c0ed3a138a3753d...537c
#
#   Three different engines, live at once, for months. Whoever sat at the table on
#   the oldest build was playing with every money defect that had been closed on the
#   others. It happened because a deploy went out from a tree that was being edited,
#   and it persisted because NOTHING EVER COMPARED THE CANISTERS TO EACH OTHER.
#
#   scripts/check-deployed.sh checks each canister against an expectation.
#   scripts/check-deployed-config.sh checks each canister against icp.yaml.
#   Both are per-canister. A fleet that is individually plausible and collectively
#   incoherent passes both. This script is the missing axis.
#
#   It also protects the verification story. README.md tells a stranger to build the
#   Docker image and compare hashes. That image builds `table_canister` ONCE, so it
#   prints one hash for the tables. Against a split fleet, a verifier who follows our
#   own instructions matches one canister of four and correctly concludes the other
#   three are not the code we published.
#
# WHAT IT DOES
#   Groups the canisters in icp.yaml by the Rust package they are built from, reads
#   each one's live module hash, and fails if any group disagrees with itself.
#
#   Optionally (--expect <file>) also checks the fleet against the hashes a
#   reproducible build produced, so "coherent" cannot mean "coherently wrong".
#
# USAGE
#   ./scripts/check-fleet-coherence.sh --network ic
#   ./scripts/check-fleet-coherence.sh --network local
#   ./scripts/check-fleet-coherence.sh --network ic --expect HASHES.txt
#
#   On --network ic it reads the PUBLIC dashboard API and needs no identity and no
#   agent call, so anyone on earth can run it against our deployment. That is the
#   point: the check a stranger runs should not require our keys.
#
#   It only ever READS. It never deploys, never writes and never moves money.
set -euo pipefail

NETWORK="local"
EXPECT=""
IDENTITY=""
while [ $# -gt 0 ]; do
  case "$1" in
    --network)  NETWORK="$2"; shift 2 ;;
    --expect)   EXPECT="$2"; shift 2 ;;
    --identity) IDENTITY="$2"; shift 2 ;;
    -h|--help)  sed -n '2,40p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

# Only the local path needs one: `icp canister status` is controller-gated, while the
# mainnet path is a keyless certified read on purpose (see module_hash below).
ID_ARG=()
[ -n "$IDENTITY" ] && ID_ARG=(--identity "$IDENTITY")

cd "$(dirname "$0")/.."

if [ "$NETWORK" = "ic" ]; then
  MAP=".icp/data/mappings/ic.ids.json"
else
  MAP=".icp/cache/mappings/local.ids.json"
fi
[ -f "$MAP" ] || { echo "no id mapping at $MAP -- is the stack deployed?" >&2; exit 1; }

# --- which canisters come from which package ------------------------------------
# Parsed out of icp.yaml rather than hardcoded, so a canister added later is covered
# without anyone remembering to edit this file. The parser lives in its own file and
# has its own self-test (`scripts/lib/package-groups.py --self-test`): the first
# version of THIS script embedded it as a heredoc inside a command substitution, the
# grouping silently came back as a single junk line, and the check reported "checked
# no package groups" instead of examining a fleet that was in fact broken. A parser
# that finds nothing must fail loudly -- that is the floor assertion at the bottom.
GROUPS_FILE="$(mktemp -t cleardeck-groups)"
trap 'rm -f "$GROUPS_FILE"' EXIT
python3 scripts/lib/package-groups.py "$MAP" > "$GROUPS_FILE"
[ -s "$GROUPS_FILE" ] || { echo "could not read any package groups out of icp.yaml" >&2; exit 1; }

# --- read one canister's live module hash ---------------------------------------
#
# ON MAINNET THIS MUST BE A CERTIFIED READ, AND IT MUST NOT BE THE DASHBOARD.
#
#   The first version of this script read https://ic-api.internetcomputer.org, because
#   it needs no key and anyone can run it. Within one hour it returned two different
#   answers for the same canister:
#
#       10:30  table_3  511c9d0e6d508d627357af22930b02daf252753621a17dd8a9fd0d5ab3796ba4
#       11:05  table_3  9c0ed3a138a3753df5f415932e7822b88f625eaf5f81d5e2c9644dd45dfd537c
#
#   Nothing was deployed in between -- every deploy in that window was `-e local`. The
#   dashboard is an INDEX over the IC, it can lag, and it is not certified. A verifier
#   who follows README.md's "compare against what the IC reports" using the dashboard
#   can conclude fraud where there is none, or all-clear where there is a split fleet.
#   Both directions are fatal to the only trust story this project has.
#
#   `dfx canister info` reads the module hash out of CERTIFIED state and needs no
#   identity (`--identity anonymous`), so a stranger can still run it. That is the
#   trade: one extra tool to install, in exchange for an answer that is signed by the
#   subnet rather than served from a cache. Do not "simplify" this back to curl.
module_hash() {
  local cid="$1"
  if [ "$NETWORK" = "ic" ]; then
    dfx canister info "$cid" --network ic --identity anonymous 2>/dev/null \
      | grep -iE 'module hash' | grep -oE '[0-9a-f]{64}' | head -1
  else
    icp canister status "$cid" -e "$NETWORK" ${ID_ARG[@]+"${ID_ARG[@]}"} 2>/dev/null \
      | grep -iE 'module hash' | grep -oE '[0-9a-f]{64}' | head -1
  fi
}

if [ "$NETWORK" = "ic" ] && ! command -v dfx >/dev/null 2>&1; then
  echo "dfx is required to read CERTIFIED module hashes on mainnet." >&2
  echo "The public dashboard API is not an acceptable substitute -- it lags, and this" >&2
  echo "check exists precisely to catch a fleet that is lying about what it runs." >&2
  echo "  sh -ci \"\$(curl -fsSL https://internetcomputer.org/install.sh)\"" >&2
  exit 1
fi

fail=0
groups_checked=0
declare -a ALL_LINES=()

echo "fleet coherence -- network: $NETWORK"
echo

while IFS=$'\t' read -r pkg names; do
  [ -n "$pkg" ] || continue
  # shellcheck disable=SC2206
  arr=($names)
  [ "${#arr[@]}" -ge 1 ] || continue

  echo "  package $pkg  (${#arr[@]} canister(s))"
  declare -a seen=()
  unreadable=0
  for name in "${arr[@]}"; do
    cid="$(python3 -c "import json;print(json.load(open('$MAP')).get('$name',''))")"
    [ -n "$cid" ] || { echo "      $name: not in $MAP"; continue; }
    h="$(module_hash "$cid")"
    if [ -z "$h" ]; then
      echo "      $name ($cid): could not read module hash"
      unreadable=1
      fail=1
      continue
    fi
    printf '      %-14s %s  %s\n' "$name" "${h:0:16}..." "$cid"
    seen+=("$h")
    ALL_LINES+=("$pkg $name $h")
  done

  # A group is coherent when every member reports the same hash.
  distinct="$(printf '%s\n' "${seen[@]+"${seen[@]}"}" | sort -u | grep -c . || true)"
  if [ "${#arr[@]}" -gt 1 ] && [ "$distinct" -gt 1 ]; then
    echo
    echo "      ✗ $distinct DIFFERENT MODULES for one package."
    echo "        These canisters are built from the same source and differ only in"
    echo "        init_args, which are not part of the module. They cannot legitimately"
    echo "        disagree. At least one of them is running code that is not this build,"
    echo "        and a player at that table is playing a different game."
    fail=1
  elif [ "$unreadable" -eq 0 ]; then
    echo "      ✓ all agree"
  fi
  groups_checked=$((groups_checked + 1))
  echo
  unset seen
done < "$GROUPS_FILE"

# --- optional: coherent AND correct ---------------------------------------------
# A fleet can agree with itself and still be three waves behind. If a reproducible
# build's HASHES.txt is supplied, every live module must appear in it.
if [ -n "$EXPECT" ]; then
  [ -f "$EXPECT" ] || { echo "--expect file not found: $EXPECT" >&2; exit 2; }
  echo "  against the reproducible build in $EXPECT"
  for line in "${ALL_LINES[@]+"${ALL_LINES[@]}"}"; do
    set -- $line
    pkg="$1"; name="$2"; live="$3"
    want="$(grep -E "^${name}[[:space:]]" "$EXPECT" | grep -oE '[0-9a-f]{64}' | head -1)"
    if [ -z "$want" ]; then
      echo "      ? $name: no expected hash in $EXPECT"
      continue
    fi
    if [ "$want" = "$live" ]; then
      printf '      ✓ %-14s matches the build\n' "$name"
    else
      printf '      ✗ %-14s live %s\n' "$name" "${live:0:16}..."
      printf '        %-14s built %s\n' "" "${want:0:16}..."
      fail=1
    fi
  done
  echo
fi

if [ "$groups_checked" -eq 0 ]; then
  echo "checked no package groups -- that is a broken check, not a pass." >&2
  exit 1
fi

if [ "$fail" -ne 0 ]; then
  echo "FLEET IS NOT COHERENT."
  echo
  echo "Fix by redeploying every backend canister from ONE build, from a clean"
  echo "committed tree, with --mode upgrade (never reinstall -- it wipes balances):"
  echo
  echo "    git status --porcelain          # must be empty. this is how it broke."
  echo "    ./scripts/deploy-mainnet.sh     # IDENTITY=<controller>"
  echo "    ./scripts/check-fleet-coherence.sh --network ic --expect HASHES.txt"
  exit 1
fi

echo "all $groups_checked package group(s) coherent."
