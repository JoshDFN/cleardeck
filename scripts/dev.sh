#!/usr/bin/env bash
#
# ClearDeck developer entry point. ONE place to bring the local stack up and to
# run every harness. `Makefile` is a thin wrapper over this file; the logic lives
# here so it is readable, greppable and callable without make.
#
#   ./scripts/dev.sh help
#
# LOCAL REPLICA ONLY. Every `icp` invocation goes through icp_local(), which
# appends `-e local`, scrubs inherited ICP_ENVIRONMENT / ICP_NETWORK, and refuses
# any argument naming a ClearDeck MAINNET canister. The mainnet canisters custody
# real ICP and ckBTC. The denylist is not duplicated here: it is read from
# .icp/data/mappings/ic.ids.json, the file the CLI itself uses for `-e ic`.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

ENV_NAME="local"
GATEWAY_HOST="${CLEARDECK_GATEWAY_HOST:-localhost}"
ICP_LEDGER_ID="ryjl3-tyaaa-aaaaa-aaaba-cai"

# Local throwaway identities. cd-local-deployer is the controller and the funder.
CONTROLLER="cd-local-deployer"
PLAYERS=(cd-alice cd-bob cd-carol cd-attacker)
# Target local ICP balance per player after `local-up`.
PLAYER_FUND_ICP="${CLEARDECK_PLAYER_FUND_ICP:-1000}"

BACKEND_CANISTERS=(history lobby table_1 table_2 table_3 btc_table_1)
TABLE_CANISTERS=(table_1 table_2 table_3 btc_table_1)

WASM_PATH="$REPO_ROOT/target/wasm32-unknown-unknown/release/table_canister.wasm"

# ---------------------------------------------------------------------------
# output helpers
# ---------------------------------------------------------------------------

if [ -t 1 ]; then B=$'\033[1m'; R=$'\033[0m'; Y=$'\033[33m'; G=$'\033[32m'; E=$'\033[31m'
else B=""; R=""; Y=""; G=""; E=""; fi

step() { printf '\n%s==> %s%s\n' "$B" "$*" "$R"; }
info() { printf '    %s\n' "$*"; }
ok()   { printf '    %s✓%s %s\n' "$G" "$R" "$*"; }
warn() { printf '    %s!%s %s\n' "$Y" "$R" "$*" >&2; }
die()  { printf '\n%sFATAL:%s %s\n' "$E" "$R" "$*" >&2; exit 1; }

require_cmd() { command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"; }

# ---------------------------------------------------------------------------
# gateway port: single source of truth is icp.yaml
# ---------------------------------------------------------------------------

gateway_port() {
  local port
  port="$(
    python3 - <<'PY' 2>/dev/null || true
import re, sys
text = open("icp.yaml").read()
# networks: -> - name: local -> gateway: -> port: N
m = re.search(r"networks:.*?-\s*name:\s*local.*?gateway:.*?port:\s*(\d+)", text, re.S)
print(m.group(1) if m else "")
PY
  )"
  printf '%s' "${port:-8000}"
}

GATEWAY_PORT="$(gateway_port)"
GATEWAY_ORIGIN="http://${GATEWAY_HOST}:${GATEWAY_PORT}"

# ---------------------------------------------------------------------------
# mainnet guard
# ---------------------------------------------------------------------------

# The authoritative list of ClearDeck mainnet canisters, read from the mapping the
# CLI resolves for `-e ic`. Never hardcode a second copy of this list.
mainnet_ids() {
  local f="$REPO_ROOT/.icp/data/mappings/ic.ids.json"
  [ -f "$f" ] || return 0
  jq -r 'to_entries[] | .value' "$f"
}

# Refuses `-e ic` / `--environment ic` in any position.
assert_not_ic_env() {
  local -a args=("$@")
  local i=1
  while [ $i -le $# ]; do
    local cur="${args[$((i-1))]}" nxt="${args[$i]:-}"
    if { [ "$cur" = "-e" ] || [ "$cur" = "--environment" ]; } && [ "$nxt" = "ic" ]; then
      die "REFUSING to run against -e ic: icp $*"
    fi
    i=$((i+1))
  done
}

# Refuses any argument that mentions a mainnet canister, anywhere inside it (so a
# Candid blob carrying a mainnet principal is caught too).
assert_no_mainnet_id() {
  local id arg
  while read -r id; do
    [ -n "$id" ] || continue
    for arg in "$@"; do
      case "$arg" in
        *"$id"*) die "REFUSING: argument names ClearDeck MAINNET canister $id (holds real funds): icp $*" ;;
      esac
    done
  done < <(mainnet_ids)
}

assert_no_mainnet() {
  assert_not_ic_env "$@"
  assert_no_mainnet_id "$@"
}

# Every icp call in this script goes through here.
icp_local() {
  assert_no_mainnet "$@"
  env -u ICP_ENVIRONMENT -u ICP_NETWORK icp "$@" -e "$ENV_NAME"
}

# ---------------------------------------------------------------------------
# replica probing
# ---------------------------------------------------------------------------

# `icp network status` reads the on-disk descriptor and reports success even when
# the replica process is gone, so liveness MUST be an HTTP probe.
gateway_is_up() {
  curl -s -m 5 -o /dev/null "${GATEWAY_ORIGIN}/api/v2/status" && return 0
  # A 400/404 from a live gateway still proves something is listening.
  local code
  code="$(curl -s -m 5 -o /dev/null -w '%{http_code}' "${GATEWAY_ORIGIN}/" || true)"
  [ -n "$code" ] && [ "$code" != "000" ]
}

# The managed network keeps one checkpoint directory per subnet. The launcher
# refuses to resume unless a single height is present on EVERY subnet. Report
# that up front instead of letting `icp network start` panic.
state_is_resumable() {
  local state_dir="$REPO_ROOT/.icp/cache/networks/${ENV_NAME}/state"
  [ -d "$state_dir" ] || return 0   # nothing to resume: a fresh start is fine
  python3 - "$state_dir" <<'PY'
import os, sys
root = sys.argv[1]
per_subnet = []
for name in sorted(os.listdir(root)):
    d = os.path.join(root, name, "checkpoints")
    if not os.path.isdir(d):
        continue
    per_subnet.append((name, set(os.listdir(d))))
if not per_subnet:
    sys.exit(0)                      # no checkpoints at all: fresh start
common = set.intersection(*[h for _, h in per_subnet])
if common:
    sys.exit(0)
print("subnet checkpoint heights do not intersect:")
for name, heights in per_subnet:
    print("  %s: %s" % (name[:12], ",".join(sorted(heights))))
sys.exit(1)
PY
}

# ---------------------------------------------------------------------------
# cmd: doctor  (read-only)
# ---------------------------------------------------------------------------

cmd_doctor() {
  step "toolchain"
  local c
  for c in icp cargo node npm jq python3 curl; do
    if command -v "$c" >/dev/null 2>&1; then
      ok "$c  $("$c" --version 2>/dev/null | head -1)"
    else
      warn "$c  MISSING"
    fi
  done
  if rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
    ok "rust target wasm32-unknown-unknown"
  else
    warn "rust target wasm32-unknown-unknown NOT installed (rustup target add wasm32-unknown-unknown)"
  fi
  if [ -x "$(dfx cache show 2>/dev/null)/pocket-ic" ]; then
    ok "pocket-ic server at $(dfx cache show)/pocket-ic"
  else
    warn "no pocket-ic server binary; the money-safety harness needs one (POCKET_IC_BIN)"
  fi

  step "local network"
  info "gateway from icp.yaml: $GATEWAY_ORIGIN"
  if gateway_is_up; then
    ok "gateway is answering"
  else
    warn "gateway is NOT answering -- the local replica is down"
    if state_is_resumable >/dev/null 2>&1; then
      info "on-disk state looks resumable: '$0 local-up' should restart it"
    else
      warn "on-disk state is NOT resumable:"
      state_is_resumable 2>&1 | sed 's/^/      /' || true
      info "'$0 local-up' will refuse to start; see 'local-up --reset' (DESTROYS local state)"
    fi
  fi

  step "local canister ids"
  local ids; ids="$(local_ids_file || true)"
  if [ -n "$ids" ]; then
    ok "$ids"
    jq -r 'to_entries[] | "      \(.key)  \(.value)"' "$ids"
    jq -e 'has("frontend")' "$ids" >/dev/null 2>&1 \
      || warn "no 'frontend' entry: the asset canister has never been created on this network"
  else
    warn "no local id mapping found; nothing has been deployed to this network"
  fi

  step "table canister wasm"
  if [ -f "$WASM_PATH" ]; then
    ok "$(shasum -a 256 "$WASM_PATH")"
    info "built $(date -r "$WASM_PATH" '+%Y-%m-%d %H:%M:%S')"
    warn "presence is not freshness: run '$0 wasm' before trusting any harness result"
  else
    warn "not built yet: run '$0 wasm'"
  fi
}

local_ids_file() {
  local f
  for f in "$REPO_ROOT/.icp/cache/mappings/${ENV_NAME}.ids.json" \
           "$REPO_ROOT/.icp/data/mappings/${ENV_NAME}.ids.json"; do
    if [ -f "$f" ] && [ "$(jq -r 'length' "$f" 2>/dev/null || echo 0)" != "0" ]; then
      printf '%s' "$f"; return 0
    fi
  done
  return 1
}

local_id() {
  local name="$1" f
  f="$(local_ids_file)" || die "no local canister id mapping; run '$0 local-up' first"
  local id; id="$(jq -r --arg n "$name" '.[$n] // empty' "$f")"
  [ -n "$id" ] || die "canister '$name' is not deployed locally (absent from $f)"
  # Belt and braces: a local mapping must never contain a mainnet id.
  local m
  while read -r m; do
    [ -n "$m" ] || continue
    [ "$id" = "$m" ] && die "local mapping for '$name' is the MAINNET id $id -- refusing"
  done < <(mainnet_ids)
  printf '%s' "$id"
}

# ---------------------------------------------------------------------------
# cmd: wasm  -- build the canister under test and print its identity
# ---------------------------------------------------------------------------
#
# Every harness result is meaningless without knowing which binary produced it.
# tests/money_safety resolves $CLEARDECK_TABLE_WASM FIRST, so exporting it here
# is what makes the money-safety suite a real gate rather than a report on
# whatever artifact happened to be lying in target/ (see docs/DEFECTS.md H-01).

cmd_wasm() {
  step "build table_canister -> wasm32-unknown-unknown/release"
  cargo build -p table_canister --target wasm32-unknown-unknown --release
  [ -f "$WASM_PATH" ] || die "cargo reported success but $WASM_PATH is absent"
  ok "$(shasum -a 256 "$WASM_PATH" | awk '{print $1}')  $WASM_PATH"
}

wasm_env() {
  cargo build -p table_canister --target wasm32-unknown-unknown --release >/dev/null
  [ -f "$WASM_PATH" ] || die "wasm build produced nothing at $WASM_PATH"
  printf 'CLEARDECK_TABLE_WASM=%s' "$WASM_PATH"
}

announce_wasm() {
  info "wasm under test: $(shasum -a 256 "$WASM_PATH" | awk '{print $1}')"
}

# ---------------------------------------------------------------------------
# cmd: local-up
# ---------------------------------------------------------------------------
#
# Idempotent. Every step checks its own postcondition. This is the recipe that
# was verified to deal a real hand on this machine; it is written down here so
# nobody has to rediscover it.

up_replica() {
  local do_reset="$1"
  if gateway_is_up; then
    ok "gateway already answering at $GATEWAY_ORIGIN (not restarting)"
    return 0
  fi
  if ! state_is_resumable >/dev/null 2>&1; then
    warn "the on-disk state of the managed network cannot be resumed:"
    state_is_resumable 2>&1 | sed 's/^/      /' || true
    if [ "$do_reset" = "0" ]; then
      die "refusing to start. Re-run with '--reset' to DELETE .icp/cache/networks/${ENV_NAME} \
(this destroys the local ledger, every deployed local canister and every local balance), \
or restore the state directory from a copy."
    fi
    warn "--reset given: deleting .icp/cache/networks/${ENV_NAME}"
    rm -rf "$REPO_ROOT/.icp/cache/networks/${ENV_NAME}" \
           "$REPO_ROOT/.icp/cache/mappings/${ENV_NAME}.ids.json"
  fi
  info "icp network start -e $ENV_NAME --background"
  icp_local network start --background
  local tries=0
  until gateway_is_up; do
    tries=$((tries+1))
    [ "$tries" -gt 60 ] && die "gateway did not come up at $GATEWAY_ORIGIN after 60s"
    sleep 1
  done
  ok "gateway up at $GATEWAY_ORIGIN"
}

# The managed network provisions the real ICP ledger at the exact id the table
# canister hardcodes as ICP_LEDGER_CANISTER, so the production deposit path works
# locally with no code changes. NEVER add a test-only faucet.
up_ledger() {
  if icp_local canister call "$ICP_LEDGER_ID" icrc1_symbol '()' --query >/dev/null 2>&1; then
    ok "real ICP ledger live at $ICP_LEDGER_ID"
    return 0
  fi
  die "no ICP ledger at $ICP_LEDGER_ID. The table canister hardcodes that id, so the deposit \
path cannot work without it. It is provisioned by the managed network itself; a network started \
without it needs '--reset'."
}

up_deploy() {
  icp_local deploy "${BACKEND_CANISTERS[@]}" --mode auto -y
  local f; f="$(local_ids_file)" || die "deploy left no local id mapping"
  ok "ids in $f"
  jq -r 'to_entries[] | "      \(.key)  \(.value)"' "$f"
}

up_wire() {
  local history_id t principal count
  history_id="$(local_id history)"
  for t in "${TABLE_CANISTERS[@]}"; do
    icp_local canister call "$t" set_history_canister \
      "(opt principal \"$history_id\")" --identity "$CONTROLLER" >/dev/null
    info "$t -> history $history_id"
  done
  principal="$(icp identity principal --identity "$CONTROLLER")"
  icp_local canister call lobby set_admin "(principal \"$principal\")" \
    --identity "$CONTROLLER" >/dev/null || warn "lobby set_admin non-zero (already set?)"
  icp_local canister call lobby init_microstakes_tables \
    "(principal \"$(local_id table_1)\", principal \"$(local_id table_2)\", principal \"$(local_id table_3)\")" \
    --identity "$CONTROLLER" >/dev/null || warn "lobby init_microstakes_tables non-zero (already initialised?)"
  count="$(icp_local canister call lobby get_tables '()' --query 2>/dev/null | grep -c 'canister_id' || true)"
  ok "lobby lists $count table record(s)"
  # btc_table_1 has no lobby registration call in any deploy path. State it rather
  # than let it look intentional. docs/DEFECTS.md T-05.
  warn "btc_table_1 is NOT registered in the lobby by any known call (docs/DEFECTS.md T-05)"
}

up_fund() {
  local id p bal
  for id in "${PLAYERS[@]}"; do
    p="$(icp identity principal --identity "$id")"
    bal="$(icp_local token balance --owner "$p" 2>/dev/null \
           | grep -oE '[0-9][0-9_.]*' | head -1 | tr -d '_' || echo 0)"
    info "$id ($p) balance=${bal:-0}"
    if [ "${bal%%.*}" -lt "$PLAYER_FUND_ICP" ] 2>/dev/null; then
      info "  topping up to ${PLAYER_FUND_ICP} ICP from $CONTROLLER"
      icp_local token transfer "$PLAYER_FUND_ICP" "$p" --identity "$CONTROLLER" >/dev/null \
        || warn "  top-up failed (is $CONTROLLER funded? managed networks pre-fund the default identity)"
    fi
  done
  ok "players funded"
}

# Delegated to the screenshot harness's builder so there is exactly ONE place that
# knows how to wire local canister ids into the bundle, and one place that PROVES
# they landed. A bare `npm run build` silently wires the bundle to the MAINNET
# fund-holding canisters (docs/DEFECTS.md T-01).
up_frontend() {
  node -e '
    import("./tools/shots/lib/frontend-build.mjs").then((m) => {
      m.buildFrontend({ log: (s) => console.log("   " + s.trim()) });
      m.deployFrontend({ log: (s) => console.log("   " + s.trim()) });
    }).catch((e) => { console.error("FATAL: " + e.message); process.exit(1); });
  '
  ok "frontend built with local ids and deployed"
}

cmd_local_up() {
  local do_reset=0 skip_frontend=0
  while [ $# -gt 0 ]; do
    case "$1" in
      --reset) do_reset=1 ;;
      --no-frontend) skip_frontend=1 ;;
      *) die "local-up: unknown option $1" ;;
    esac
    shift
  done

  require_cmd icp; require_cmd jq; require_cmd curl; require_cmd cargo; require_cmd node

  step "[1/6] replica";              up_replica "$do_reset"
  step "[2/6] ICP ledger";           up_ledger
  step "[3/6] deploy backend";       up_deploy
  step "[4/6] wire history + lobby"; up_wire
  step "[5/6] fund local players";   up_fund
  step "[6/6] frontend"
  if [ "$skip_frontend" = "1" ]; then info "skipped (--no-frontend)"; else up_frontend; fi

  step "local stack is up"
  info "gateway     $GATEWAY_ORIGIN"
  info "ledger      $ICP_LEDGER_ID (real ICP ledger)"
  info "next        $0 test"
}

cmd_local_status() { cmd_doctor; }

# ---------------------------------------------------------------------------
# cmd: test  -- the fast gate
# ---------------------------------------------------------------------------
#
# Deliberately excludes: the long fuzz run, the exhaustive evaluator sweep, the
# screenshot harness (needs a replica) and the known-defect markers (red on
# purpose). Everything here runs with no replica and no network access beyond a
# cached ledger download.

cmd_test() {
  local failed=()

  step "[1/4] cargo test --workspace"
  cargo test --workspace || failed+=("cargo test --workspace")

  step "[2/4] table_canister wasm build"
  cmd_wasm || failed+=("wasm build")

  step "[3/4] differential fast subset (tools/differential)"
  ( cd tools/differential && cargo test ) || failed+=("differential fast subset")

  step "[4/4] money-safety fast subset (tests/money_safety)"
  announce_wasm
  (
    cd tests/money_safety
    export CLEARDECK_TABLE_WASM="$WASM_PATH"
    cargo test --test invariants  -- --test-threads=2 &&
    cargo test --test regressions -- --test-threads=2 &&
    MONEY_FUZZ_SEEDS="${CLEARDECK_SMOKE_FUZZ_SEEDS:-1}" \
    MONEY_FUZZ_STEPS="${CLEARDECK_SMOKE_FUZZ_STEPS:-40}" \
    MONEY_FUZZ_SHRINK=10 \
      cargo test --test fuzz
  ) || failed+=("money-safety fast subset")

  step "result"
  if [ ${#failed[@]} -eq 0 ]; then
    ok "all fast gates green"
    return 0
  fi
  printf '    %sFAILED:%s %s\n' "$E" "$R" "${failed[*]}" >&2
  return 1
}

# ---------------------------------------------------------------------------
# cmd: fuzz  -- the long hostile-sequence run
# ---------------------------------------------------------------------------

cmd_fuzz() {
  step "money-safety hostile-sequence fuzzer (long)"
  cargo build -p table_canister --target wasm32-unknown-unknown --release >/dev/null
  announce_wasm
  local seeds steps
  seeds="${MONEY_FUZZ_SEEDS:-$(printf '%s' \
    "212967420072193,212967420072194,212967420072195,212967420072196,212967420072197,212967420072198,212967420072199,212967420072200,212967420072201")}"
  steps="${MONEY_FUZZ_STEPS:-600}"
  info "seeds: $seeds"
  info "steps per seed: $steps  (each step is a real IC message; expect minutes)"
  (
    cd tests/money_safety
    export CLEARDECK_TABLE_WASM="$WASM_PATH"
    MONEY_FUZZ_SEEDS="$seeds" MONEY_FUZZ_STEPS="$steps" \
      cargo test --test fuzz -- --nocapture
  )
  info "machine-readable report: target/money-safety/money-fuzz-report.json"
}

# ---------------------------------------------------------------------------
# cmd: diff-full  -- exhaustive evaluator differential
# ---------------------------------------------------------------------------

PHE_VENV="$REPO_ROOT/target/phe-venv"

cmd_phe_venv() {
  step "third reference (phevaluator) virtualenv"
  if [ -x "$PHE_VENV/bin/python" ] && "$PHE_VENV/bin/python" -c 'import phevaluator' 2>/dev/null; then
    ok "already present: $PHE_VENV"
    return 0
  fi
  python3 -m venv "$PHE_VENV"
  "$PHE_VENV/bin/pip" install --quiet phevaluator
  "$PHE_VENV/bin/python" -c 'import phevaluator' || die "phevaluator did not import after install"
  ok "installed: $PHE_VENV"
}

cmd_diff_full() {
  cmd_phe_venv
  step "exhaustive differential: all C(52,5) five-card hands x 3 evaluators"
  info "plus 5,000,000 random seven-card hands and 4,000,000 pairs"
  info "add --exhaustive-sevens for all C(52,7) (~20 min more)"
  (
    cd tools/differential
    CLEARDECK_PHE_PYTHON="$PHE_VENV/bin/python" \
    CLEARDECK_DIFF_REPORT="${CLEARDECK_DIFF_REPORT:-$REPO_ROOT/target/differential-report.json}" \
      cargo run --release -- "$@"
  )
}

# ---------------------------------------------------------------------------
# cmd: shots  -- screenshots of the real UI against the real canisters
# ---------------------------------------------------------------------------

cmd_shots() {
  step "screenshot harness"
  gateway_is_up || die "the local replica gateway at $GATEWAY_ORIGIN is not answering. \
Run '$0 local-up' first. This harness deliberately does not start the replica."
  if [ ! -d tools/shots/node_modules/playwright ]; then
    info "installing playwright in tools/shots"
    ( cd tools/shots && npm install --no-audit --no-fund )
  fi
  node tools/shots/run.mjs "$@"
}

# ---------------------------------------------------------------------------
# cmd: known-defects  -- the markers that are RED on purpose
# ---------------------------------------------------------------------------
#
# Five tests in tools/differential are #[ignore]d and written to FAIL until the
# engine is fixed. They are NOT part of `test`, because a suite that is red by
# design teaches everyone to ignore red. This target inverts them: it succeeds
# while they still fail, and tells you the moment one goes green.

DEFECT_MARKERS=(
  defect_detect_straight_returns_the_best_straight
  defect_duplicate_cards_are_rejected
  defect_evaluate_hand_rejects_duplicate_cards
  defect_short_board_is_fixed
  defect_evaluate_five_cards_rejects_more_than_five_cards
)

cmd_known_defects() {
  step "known-defect markers (expected RED until wave 2 fixes them)"
  local m out still_red=0 broken=0 now_green=()
  for m in "${DEFECT_MARKERS[@]}"; do
    out="$( ( cd tools/differential && cargo test --release --test fast_subset \
              -- --ignored --exact "$m" ) 2>&1 || true )"
    # A marker that runs ZERO tests must never be read as "fixed": that is a
    # renamed or un-#[ignore]d test, not a green defect.
    if printf '%s' "$out" | grep -q '1 passed; 0 failed'; then
      now_green+=("$m")
      printf '    %sGREEN%s %s  <-- defect appears FIXED\n' "$G" "$R" "$m"
    elif printf '%s' "$out" | grep -q '0 passed; 1 failed'; then
      still_red=$((still_red+1))
      printf '    %sred%s   %s\n' "$Y" "$R" "$m"
    else
      broken=$((broken+1))
      printf '    %s?%s     %s  <-- ran 0 tests: renamed, deleted or no longer #[ignore]d\n' \
        "$E" "$R" "$m"
    fi
  done
  step "result"
  info "$still_red of ${#DEFECT_MARKERS[@]} engine defects still present"
  if [ ${#now_green[@]} -gt 0 ]; then
    ok "${#now_green[@]} defect(s) now fixed: ${now_green[*]}"
    info "un-#[ignore] them in tools/differential/tests/fast_subset.rs and update docs/DEFECTS.md"
  fi
  if [ "$broken" -gt 0 ]; then
    warn "$broken marker(s) could not be resolved; DEFECT_MARKERS in this script is stale"
    return 1
  fi
  return 0
}

# ---------------------------------------------------------------------------
# cmd: hygiene  -- the repo-state checks wave 1 had to do by hand
# ---------------------------------------------------------------------------

BASELINE_COMMIT="${CLEARDECK_BASELINE:-ceacc37}"

# Substrings that must never be weakened, shortened or relocated out of sight.
# Making them MORE prominent is always allowed, so these are presence checks and
# a diff against the baseline, not equality checks on the whole file.
#
# The two surfaces word the unaudited warning differently ("unaudited alpha
# software" in README.md, "Unaudited code with known bugs" in the app), so the
# lists are per-surface rather than one list applied to both.
declare -a README_NOTICES=(
  "unaudited alpha software"
  "your funds are NOT safe"
  "18+ only"
  "illegal in many jurisdictions"
)
declare -a FRONTEND_NOTICES=(
  "Unaudited code with known bugs"
  "your funds are NOT safe"
  "18+ only"
  "illegal in many jurisdictions"
)

cmd_hygiene() {
  local bad=0

  step "no large or binary files added"
  local n bytes
  n="$(git status --porcelain --untracked-files=all | wc -l | tr -d ' ')"
  bytes="$(git status --porcelain --untracked-files=all | sed 's/^...//' \
           | while read -r f; do [ -f "$f" ] && wc -c <"$f"; done | awk '{s+=$1} END {print s+0}')"
  info "$n untracked path(s), $((bytes/1024)) KiB total"
  if git status --porcelain --untracked-files=all | sed 's/^...//' \
       | grep -Eiq '\.(png|jpg|jpeg|gif|webp|webm|mp4|wasm|gz|zip|bin|pdf|tiff|bmp)$'; then
    warn "binary/media files are staged for commit:"
    git status --porcelain --untracked-files=all | sed 's/^...//' \
      | grep -Ei '\.(png|jpg|jpeg|gif|webp|webm|mp4|wasm|gz|zip|bin|pdf|tiff|bmp)$' | sed 's/^/      /'
    bad=1
  else
    ok "no binary or image files would be committed"
  fi
  if [ "$bytes" -gt $((4 * 1024 * 1024)) ]; then
    warn "untracked payload exceeds 4 MiB; check for a stray artifact directory"
    bad=1
  fi

  step "player-protection notices intact"
  local notice
  for notice in "${README_NOTICES[@]}"; do
    if grep -qF "$notice" README.md; then ok "README.md: \"$notice\""
    else warn "README.md is MISSING \"$notice\""; bad=1; fi
  done
  for notice in "${FRONTEND_NOTICES[@]}"; do
    if grep -rqF "$notice" src/cleardeck_frontend/src; then ok "frontend:  \"$notice\""
    else warn "frontend is MISSING \"$notice\""; bad=1; fi
  done
  if grep -qF "No Rake" README.md; then ok "README.md: no-rake property stated"
  else warn "README.md no longer states the no-rake property"; bad=1; fi

  step "notices not weakened since $BASELINE_COMMIT"
  local removed
  removed="$(git diff "$BASELINE_COMMIT" -- README.md src/cleardeck_frontend/src \
             | grep '^-' | grep -Ei 'unaudited|18\+|jurisdiction|rake' || true)"
  if [ -n "$removed" ]; then
    warn "lines containing a protection notice were REMOVED or CHANGED:"
    printf '%s\n' "$removed" | sed 's/^/      /'
    bad=1
  else
    ok "no notice line removed or altered"
  fi

  step "result"
  if [ "$bad" = "0" ]; then ok "repo hygiene clean"; return 0; fi
  warn "repo hygiene FAILED"; return 1
}

# ---------------------------------------------------------------------------
# cmd: selftest  -- prove the mainnet guard actually refuses
# ---------------------------------------------------------------------------
#
# A guard nobody exercises is decoration. This asserts that every hostile shape
# is refused, using the real mainnet ids from .icp/data/mappings/ic.ids.json.

cmd_selftest() {
  local fails=0

  must_refuse() {
    local desc="$1"; shift
    if ( assert_no_mainnet "$@" ) >/dev/null 2>&1; then
      printf '    %sLEAK%s  %s  --  args: %s\n' "$E" "$R" "$desc" "$*" >&2
      fails=$((fails+1))
    else
      ok "refused: $desc"
    fi
  }
  must_allow() {
    local desc="$1"; shift
    if ( assert_no_mainnet "$@" ) >/dev/null 2>&1; then
      ok "allowed: $desc"
    else
      printf '    %sFALSE POSITIVE%s  %s  --  args: %s\n' "$E" "$R" "$desc" "$*" >&2
      fails=$((fails+1))
    fi
  }

  step "mainnet guard"
  must_refuse "-e ic"                 canister call lobby get_tables -e ic
  must_refuse "--environment ic"      canister call lobby get_tables --environment ic
  must_refuse "-e ic in first slot"   -e ic canister call lobby get_tables
  local m
  while read -r m; do
    [ -n "$m" ] || continue
    must_refuse "mainnet id $m as target"        canister call "$m" get_table_view
    must_refuse "mainnet id $m inside an arg"    canister call table_1 x "(principal \"$m\")"
  done < <(mainnet_ids)
  must_allow "a local id"             canister call "$(jq -r '.lobby' "$(local_ids_file)" 2>/dev/null || echo aaaaa-aa)" get_tables
  must_allow "the local ICP ledger"   canister call "$ICP_LEDGER_ID" icrc1_symbol
  must_allow "the word ic in a path"  deploy --project-root-override /tmp/ic-thing

  step "mainnet id list is non-empty"
  local n; n="$(mainnet_ids | grep -c . || true)"
  if [ "$n" -ge 7 ]; then ok "$n mainnet ids in the denylist"
  else printf '    %sEMPTY DENYLIST%s: only %s id(s) read from ic.ids.json\n' "$E" "$R" "$n" >&2; fails=$((fails+1)); fi

  step "result"
  if [ "$fails" = "0" ]; then ok "guard selftest passed"; return 0; fi
  warn "guard selftest FAILED with $fails problem(s)"; return 1
}

# ---------------------------------------------------------------------------
# cmd: help
# ---------------------------------------------------------------------------

cmd_help() {
  cat <<EOF
${B}ClearDeck dev entry point${R}   (make <target> works for all of these)

  ${B}doctor${R}          read-only: toolchain, replica liveness, deployed ids, wasm identity
  ${B}local-up${R}        bring the local stack up end to end (idempotent)
                    --reset        DELETE unresumable local network state first
                    --no-frontend  skip the frontend build/deploy
  ${B}local-status${R}    alias for doctor
  ${B}wasm${R}            build table_canister.wasm and print its sha256

  ${B}test${R}            FAST gate: workspace tests + wasm build + differential fast
                  subset + money-safety invariants/regressions + a short fuzz run.
                  No replica needed.
  ${B}fuzz${R}            LONG: 9 seeds x 600 hostile steps against the real canister
                  and the real ICP ledger on PocketIC
  ${B}diff-full${R}       exhaustive evaluator differential (all C(52,5) x 3 evaluators)
  ${B}shots${R}           screenshot the real UI against the real local canisters
                  (requires local-up)
  ${B}known-defects${R}   run the markers that are RED on purpose; succeeds while the
                  engine defects are still present, shouts when one is fixed
  ${B}hygiene${R}         no large/binary files added; disclaimer, 18+, jurisdiction and
                  no-rake notices present and not weakened since ${BASELINE_COMMIT}
  ${B}selftest${R}        prove the mainnet guard refuses every hostile argument shape

  ${B}phe-venv${R}        install the third reference evaluator (phevaluator) locally

Everything targets the LOCAL environment only. Mainnet ids from
.icp/data/mappings/ic.ids.json are refused in every icp invocation.
EOF
}

# ---------------------------------------------------------------------------

main() {
  local cmd="${1:-help}"; shift || true
  case "$cmd" in
    help|-h|--help) cmd_help ;;
    doctor)         cmd_doctor ;;
    local-up)       cmd_local_up "$@" ;;
    local-status)   cmd_local_status ;;
    wasm)           cmd_wasm ;;
    test)           cmd_test ;;
    fuzz)           cmd_fuzz ;;
    diff-full)      cmd_diff_full "$@" ;;
    shots)          cmd_shots "$@" ;;
    known-defects)  cmd_known_defects ;;
    hygiene)        cmd_hygiene ;;
    selftest)       cmd_selftest ;;
    phe-venv)       cmd_phe_venv ;;
    *) die "unknown command '$cmd' (try '$0 help')" ;;
  esac
}

main "$@"
