#!/usr/bin/env bash
# Every copy of a pinned tool version must agree with every other copy.
#
# WHY THIS EXISTS
#   The compiler, ic-wasm and icp-cli versions are each declared in three or four
#   separate files, and the only thing keeping them equal was a comment saying
#   "keep in sync". They were not in sync:
#
#     * .github/workflows/deploy-ic.yml ran `cargo install ic-wasm --locked`
#       with NO VERSION, in a step titled "Install pinned toolchain". CI proves
#       the build reproducible with ic-wasm 0.9.9; the mainnet deploy used
#       whatever crates.io served that morning. The Dockerfile says why that
#       matters, in its own words: "a different ic-wasm shrinks differently and
#       the module hash moves."
#     * deploy-ic.yml and cycles-monitor.yml pinned icp-cli 1.0.0 while ci.yml
#       and the Dockerfile pinned 1.0.2.
#
#   The whole verifiability claim of this project is "the deployed module hash
#   equals a reproducible build of main". Deploying with tools other than the
#   ones CI reproduced the build with can break that silently, and the only
#   symptom is an auditor who cannot match the hash -- which has happened here
#   before.
#
# USAGE
#   ./scripts/check-declarations.sh
#   It only reads files in this repository. No network, no build.
set -euo pipefail
cd "$(dirname "$0")/.."

fail=0
checked=0

# Print every DECLARED value of one pinned thing, as "<value>\t<where>".
# Each extractor names the exact file and the exact syntax it reads, so adding a
# new copy somewhere else shows up as a missing source rather than silent drift.
declare_rust() {
  sed -n 's/^channel *= *"\([^"]*\)".*/\1\trust-toolchain.toml/p' rust-toolchain.toml
  for w in .github/workflows/*.yml; do
    sed -n "s|^ *RUST_TOOLCHAIN: *'\([^']*\)'.*|\1\t$w|p" "$w"
  done
  sed -n 's/^FROM.*rust:\([0-9][0-9.]*\)-slim.*/\1\tDockerfile/p' Dockerfile
}

declare_ic_wasm() {
  sed -n 's/^ARG IC_WASM_VERSION=\(.*\)/\1\tDockerfile/p' Dockerfile
  for w in .github/workflows/*.yml; do
    sed -n "s|^ *IC_WASM_VERSION: *'\([^']*\)'.*|\1\t$w|p" "$w"
  done
  sed -n "s|.*cargo install ic-wasm --version \([0-9][0-9.]*\) --locked.*|\1\trecipes/rust-reproducible.hbs|p" \
    recipes/rust-reproducible.hbs
}

declare_icp_cli() {
  sed -n 's/^ARG ICP_CLI_VERSION=\(.*\)/\1\tDockerfile/p' Dockerfile
  for w in .github/workflows/*.yml; do
    sed -n "s|^ *ICP_CLI_VERSION: *'\([^']*\)'.*|\1\t$w|p" "$w"
  done
}

declare_candid_extractor() {
  sed -n "s|^CANDID_EXTRACTOR_VERSION='\([^']*\)'.*|\1\tscripts/check-candid.sh|p" scripts/check-candid.sh
  for w in .github/workflows/*.yml; do
    sed -n "s|.*cargo install candid-extractor --version \([0-9][0-9.]*\) --locked.*|\1\t$w|p" "$w"
  done
}

# An UNPINNED install is drift by definition: it declares "latest", which is a
# different thing on different days. These greps must find nothing.
check_unpinned() {
  local hits
  hits="$(grep -rn "cargo install \(ic-wasm\|candid-extractor\)" .github/workflows/ \
            | grep -v -- '--version' || true)"
  if [ -n "$hits" ]; then
    echo "  ✗ a tool that changes the module hash is installed WITHOUT a version:"
    printf '%s\n' "$hits" | sed 's/^/      /'
    fail=1
  else
    echo "  ✓ every ic-wasm / candid-extractor install in a workflow is pinned"
  fi
  hits="$(grep -rn "icp-sdk/icp-cli@" .github/workflows/ Dockerfile \
            | grep -v 'ICP_CLI_VERSION' || true)"
  if [ -n "$hits" ]; then
    echo "  ✗ icp-cli installed without going through ICP_CLI_VERSION:"
    printf '%s\n' "$hits" | sed 's/^/      /'
    fail=1
  else
    echo "  ✓ every icp-cli install goes through ICP_CLI_VERSION"
  fi
}

compare() {
  local label="$1" fn="$2" min="$3"
  local out distinct n
  out="$("$fn")"
  n="$(printf '%s\n' "$out" | grep -c . || true)"
  # A extractor that silently matches nothing would make every version "agree".
  if [ "$n" -lt "$min" ]; then
    echo "  ✗ $label: found only $n declaration(s), expected at least $min --"
    echo "      the extractor in this script has gone stale, which makes the"
    echo "      comparison vacuous. Fix the extractor, do not lower the bound."
    fail=1
    return
  fi
  distinct="$(printf '%s\n' "$out" | cut -f1 | sort -u | grep -c . || true)"
  checked=$((checked + 1))
  if [ "$distinct" -eq 1 ]; then
    printf '  ✓ %-18s %s  (%s copies agree)\n' "$label" "$(printf '%s\n' "$out" | head -1 | cut -f1)" "$n"
  else
    echo "  ✗ $label: $distinct DIFFERENT versions declared"
    printf '%s\n' "$out" | sed 's/^/      /'
    fail=1
  fi
}

# Mainnet canister IDs are written down in at least four places: two committed
# JSON files, and twice more inside deploy-ic.yml (once as the build-time env the
# frontend bundle is compiled with, once as the grep that asserts the ID reached
# the bundle). A frontend built against the wrong lobby ID is a site that talks
# to nothing, and the grep guard would happily confirm the wrong ID.
check_canister_ids() {
  local bad=0 n
  for n in lobby history table_1 table_2 table_3 btc_table_1 frontend; do
    local a b
    a="$(python3 -c "import json;print(json.load(open('canister_ids.json')).get('$n',{}).get('ic',''))")"
    b="$(python3 -c "import json;print(json.load(open('.icp/data/mappings/ic.ids.json')).get('$n',''))")"
    if [ -z "$a" ] || [ -z "$b" ]; then
      echo "  ✗ $n: missing from canister_ids.json ('$a') or ic.ids.json ('$b')"
      bad=1
    elif [ "$a" != "$b" ]; then
      echo "  ✗ $n: canister_ids.json says $a, .icp/data/mappings/ic.ids.json says $b"
      bad=1
    fi
  done
  # The IDs hardcoded into the deploy workflow must be the same ones.
  local want_lobby want_history
  want_lobby="$(python3 -c "import json;print(json.load(open('.icp/data/mappings/ic.ids.json'))['lobby'])")"
  want_history="$(python3 -c "import json;print(json.load(open('.icp/data/mappings/ic.ids.json'))['history'])")"
  local w=.github/workflows/deploy-ic.yml
  local seen_lobby
  seen_lobby="$(grep -c "$want_lobby" "$w" || true)"
  if [ "$seen_lobby" -lt 2 ]; then
    echo "  ✗ $w mentions the live lobby ID $want_lobby only $seen_lobby time(s);"
    echo "      it must appear as VITE_CANISTER_ID_LOBBY and in the bundle grep guard"
    bad=1
  fi
  if ! grep -q "VITE_CANISTER_ID_HISTORY: $want_history" "$w"; then
    echo "  ✗ $w does not build the frontend against history $want_history"
    bad=1
  fi
  # README is where a stranger copies an ID from. Any canister-shaped id in it
  # must be one this project actually runs -- a stale one sends a reader, and
  # possibly their money, at somebody else's canister.
  local live stale
  live="$(python3 -c "import json;print(' '.join(json.load(open('.icp/data/mappings/ic.ids.json')).values()))")"
  stale=""
  for id in $(grep -oE '\b[a-z0-9]{5}-[a-z0-9]{5}-[a-z0-9]{5}-[a-z0-9]{5}-cai\b' README.md | sort -u); do
    case " $live " in
      *" $id "*) ;;
      *) stale="$stale $id" ;;
    esac
  done
  if [ -n "$stale" ]; then
    echo "  ✗ README.md names canister id(s) this project does not run:$stale"
    bad=1
  fi

  if [ "$bad" -eq 0 ]; then
    echo "  ✓ canister IDs agree across canister_ids.json, ic.ids.json, deploy-ic.yml and README.md"
  else
    fail=1
  fi
}

echo "== mainnet canister IDs declared in more than one place"
check_canister_ids
echo
echo "== pinned tool versions declared in more than one place"
compare "rust toolchain"    declare_rust             5
compare "ic-wasm"           declare_ic_wasm          3
compare "icp-cli"           declare_icp_cli          4
compare "candid-extractor"  declare_candid_extractor 2
echo
echo "== nothing that moves the module hash may be installed unpinned"
check_unpinned

echo
if [ "$fail" -ne 0 ]; then
  cat <<'EOF'
DECLARED TOOL VERSIONS DISAGREE.

This project's verifiability claim is that the deployed module hash equals a
reproducible build of main. That only holds if the deploy uses the same tools CI
reproduced the build with. Make every copy equal, or the hash an auditor computes
will not be the hash that is running.
EOF
  exit 1
fi
echo "all $checked pinned versions agree across every file that declares them"
