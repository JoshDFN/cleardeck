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

# Kept in step with `DEFAULT_STEPS` in tests/money_safety/src/fuzz.rs. Only used to
# print what `fuzz-default` is about to do; the run itself passes NO environment.
DEFAULT_FUZZ_STEPS=220

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

# HOW THE LOCAL MODULES WERE BUILT, WRITTEN DOWN WHERE THE VERIFIER CAN READ IT.
#
# docs/DEFECTS.md E-58. `verify-build.sh --local` used to rebuild in the
# digest-pinned linux/amd64 container and compare that against canisters this
# function had just installed from a native macOS build. Those two are not
# byte-identical and never can be -- rustc emits different wasm per host OS and
# architecture, which the Dockerfile has documented all along -- so the
# documented pair of commands, `local-up` then `verify-build.sh --local`,
# produced 6 of 6 MISMATCH on a stack that was in fact perfectly consistent. An
# auditor followed the instructions literally and read that as possible
# tampering, which is exactly the wrong signal from a verification tool.
#
# The two paths now agree because the verifier stops guessing: this note says
# which builder produced the modules that are installed, and the verifier
# rebuilds the same way.
PROVENANCE_FILE="$REPO_ROOT/.icp/cache/cleardeck-build-provenance.txt"
write_provenance() {
  local builder="$1"
  mkdir -p "$(dirname "$PROVENANCE_FILE")"
  {
    printf 'builder=%s\n' "$builder"
    printf 'written_at=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    printf 'revision=%s\n' "$(git rev-parse HEAD 2>/dev/null || echo unknown)"
    printf 'dirty=%s\n' "$([ -n "$(git status --porcelain 2>/dev/null)" ] && echo dirty || echo clean)"
    printf 'cargo=%s\n' "$(cargo --version 2>/dev/null || echo unknown)"
    printf 'uname=%s\n' "$(uname -sm)"
    printf '# read by scripts/verify-build.sh --local to pick a builder (docs/DEFECTS.md E-58)\n'
  } > "$PROVENANCE_FILE"
}

# $1 = "host" | "docker"
up_deploy() {
  local builder="${1:-host}" emitted name

  # Always run the ordinary deploy first: it creates the canisters, applies the
  # init args from icp.yaml and installs a working module. The container path
  # then UPGRADES each canister to the container-built bytes, which keeps the
  # init-arg handling in exactly one place (the manifest) instead of duplicating
  # it here where it would drift.
  icp_local deploy "${BACKEND_CANISTERS[@]}" --mode auto -y

  if [ "$builder" = docker ]; then
    step "[3b/6] swap in container-built modules"
    local ctl ctl_flag=()
    ctl="$(resolve_controller_identity lobby)" \
      || die "no local icp identity controls the backend canisters, so they cannot be upgraded"
    ctl_flag=(--identity "$ctl")
    emitted="$(mktemp -d -t cleardeck-emit)"
    "$REPO_ROOT/scripts/verify-build.sh" --emit "$emitted" \
      || die "the container build failed; the local stack is up on host-built modules"
    for name in "${BACKEND_CANISTERS[@]}"; do
      call_or_die "$name install (container-built)" \
        icp_local canister install "$name" --wasm "$emitted/$name.wasm" \
          --mode upgrade -y "${ctl_flag[@]}" >/dev/null
      info "$name <- $(basename "$emitted")/$name.wasm"
    done
    rm -rf "$emitted"
    ok "every backend canister is running the linux/amd64 container build (as $ctl)"
  fi

  write_provenance "$builder"

  local f; f="$(local_ids_file)" || die "deploy left no local id mapping"
  ok "ids in $f"
  jq -r 'to_entries[] | "      \(.key)  \(.value)"' "$f"
  info "build provenance recorded in $(basename "$PROVENANCE_FILE"): builder=$builder"
  info "verify with: ./scripts/verify-build.sh --local"
}

# `icp canister call` EXITS ZERO when the method returns `variant { Err = ... }`.
# The Candid Result is a value, not a transport failure, so `cmd >/dev/null ||
# die` cannot see a refusal. Every controller-only call in this file went through
# that hole. Route them here instead: the reply is read, an `Err` is fatal, and
# the message the canister actually gave is printed.
call_or_die() {
  local what="$1"; shift
  local out
  if ! out="$("$@" 2>&1)"; then
    die "$what: the call itself failed
      $out"
  fi
  case "$out" in
    *"variant { Err"*|*"Err ="*)
      die "$what: the canister REFUSED it (exit status was still 0)
      $out" ;;
  esac
  printf '%s' "$out"
}

# Which local identity actually controls $1.
#
# `$CONTROLLER` is a documented claim, and on this machine it was false: the
# backend was deployed by the default identity, so every
# `--identity cd-local-deployer` controller call returned
# `Err("Unauthorized: controller access required")` and exited 0. Resolve it
# from the canister instead of asserting it. `icp identity list` prints
# name and principal on one line, so one call maps them all.
resolve_controller_identity() {
  local canister="$1" controllers name principal
  controllers="$(icp_local canister status "$canister" 2>/dev/null \
                 | awk -F': ' '/^[[:space:]]*Controllers:/ {print $2}')"
  [ -n "$controllers" ] || return 1
  # Prefer the documented identity when it really is a controller.
  for name in "$CONTROLLER" $(icp identity list 2>/dev/null \
                              | sed 's/^\*\{0,1\}[[:space:]]*//' | awk 'NF>=2 {print $1}'); do
    principal="$(icp identity principal --identity "$name" 2>/dev/null)" || continue
    case " $controllers " in
      *" $principal "*) printf '%s' "$name"; return 0 ;;
    esac
  done
  return 1
}

# Wiring the archive is TWO calls in opposite directions, and shipping only one
# of them is invisible until somebody looks for a hand from last week.
#
# The table needs `set_history_canister` so it knows where to send. The archive
# needs `authorize_table` so it will accept what arrives. This function used to
# make only the first call, as an identity that is not a controller, discarding
# stdout and never reading the `Err` in the reply. It then printed
# "table_1 -> history <id>" and returned success. The result held for the whole
# of wave 5: every table pointed nowhere, the archive was authorised for no
# tables, `record_hand` had never once been accepted, and `get_total_hands`
# returned 0 while the README called the archive "permanent hand history"
# (docs/DEFECTS.md T-34).
#
# So: the controller is resolved, not assumed; both directions are called; every
# reply is read; and the wiring is CHECKED by reading it back off both canisters
# afterwards. A silent archive is worth less than no archive, because no archive
# does not claim to be one.
up_wire() {
  local history_id t principal count table_principal authorized ctl
  history_id="$(local_id history)"

  ctl="$(resolve_controller_identity table_1)" \
    || die "no local icp identity controls table_1, so the archive cannot be wired. \
Controllers: $(icp_local canister status table_1 2>/dev/null | awk -F': ' '/Controllers:/{print $2}')"
  if [ "$ctl" != "$CONTROLLER" ]; then
    warn "the controller is '$ctl', NOT the documented '$CONTROLLER'. Using '$ctl'."
  fi

  for t in "${TABLE_CANISTERS[@]}"; do
    table_principal="$(local_id "$t")"
    call_or_die "$t set_history_canister" \
      icp_local canister call "$t" set_history_canister \
        "(opt principal \"$history_id\")" --identity "$ctl" >/dev/null
    call_or_die "history authorize_table($t)" \
      icp_local canister call history authorize_table \
        "(principal \"$table_principal\")" --identity "$ctl" >/dev/null
    info "$t ($table_principal) <-> history $history_id"
  done

  # Postconditions, read back off the canisters. A call that returned Ok is not
  # the same fact as the wiring being in place.
  for t in "${TABLE_CANISTERS[@]}"; do
    icp_local canister call "$t" get_history_canister '()' --query \
      | grep -qF "$history_id" \
      || die "$t get_history_canister does not report $history_id after wiring"
  done
  authorized="$(icp_local canister call history get_authorized_tables '()' --query)"
  for t in "${TABLE_CANISTERS[@]}"; do
    table_principal="$(local_id "$t")"
    printf '%s' "$authorized" | grep -qF "$table_principal" \
      || die "history does not list $t ($table_principal) as authorised after wiring"
  done
  ok "archive wired both ways and verified: ${#TABLE_CANISTERS[@]} table(s) <-> history $history_id"

  # Hands that settled while the archive was unreachable are still held by the
  # tables. Push them now, so bringing the stack up repairs the record instead
  # of only fixing it going forward.
  for t in "${TABLE_CANISTERS[@]}"; do
    icp_local canister call "$t" flush_unrecorded_hands '()' --identity "$ctl" >/dev/null 2>&1 || true
  done

  # The lobby wiring below is left exactly as it was, including its use of
  # $CONTROLLER and its `|| warn`. The same blind spot applies to it -- an
  # Err in the reply still exits 0 -- but the lobby is another agent's file
  # this wave and a silent behaviour change here would be worse than a
  # documented one. Filed in docs/DEFECTS.md.
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
  local do_reset=0 skip_frontend=0 builder=host
  while [ $# -gt 0 ]; do
    case "$1" in
      --reset) do_reset=1 ;;
      --no-frontend) skip_frontend=1 ;;
      # Deploy the modules the reproducible-build image produces, instead of the
      # ones your own toolchain produces. Slow (an emulated linux/amd64 build)
      # and unnecessary for development, but it makes the local replica a real
      # rehearsal of the mainnet verification: after this,
      # `./scripts/verify-build.sh --local` runs the same container check a
      # stranger runs against mainnet, end to end, on a stack you control.
      --docker) builder=docker ;;
      *) die "local-up: unknown option $1" ;;
    esac
    shift
  done

  require_cmd icp; require_cmd jq; require_cmd curl; require_cmd cargo; require_cmd node
  [ "$builder" = docker ] && { require_cmd docker; docker info >/dev/null 2>&1 \
    || die "--docker needs the Docker daemon running"; }

  step "[1/6] replica";              up_replica "$do_reset"
  step "[2/6] ICP ledger";           up_ledger
  step "[3/6] deploy backend";       up_deploy "$builder"
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

# Run "$@" but kill it after $1 seconds. macOS has no coreutils `timeout`, and the
# settlement oracle can HANG rather than fail (docs/DEFECTS.md H-22: its
# rewind-and-re-deal search is unbounded, so a deck that stops varying spins
# forever). A gate that hangs is worse than one that fails, because a hang looks
# like a slow job.
with_timeout() {
  local secs="$1"; shift
  ( "$@" ) & local pid=$!
  ( sleep "$secs"; kill -9 "$pid" 2>/dev/null ) & local wd=$!
  wait "$pid"; local rc=$?
  kill "$wd" 2>/dev/null
  if [ "$rc" -ge 128 ]; then
    printf '    %sTIMED OUT%s after %ss: %s\n' "$E" "$R" "$secs" "$*" >&2
  fi
  return "$rc"
}

cmd_test() {
  local failed=()

  step "[1/6] cargo test --workspace"
  cargo test --workspace || failed+=("cargo test --workspace")

  step "[2/6] table_canister wasm build"
  cmd_wasm || failed+=("wasm build")

  step "[3/6] differential fast subset (tools/differential)"
  ( cd tools/differential && cargo test ) || failed+=("differential fast subset")

  step "[4/6] money-safety fast subset + the fuzzer at its own defaults"
  announce_wasm
  (
    cd tests/money_safety
    export CLEARDECK_TABLE_WASM="$WASM_PATH"
    cargo test --test invariants  -- --test-threads=2 &&
    cargo test --test regressions -- --test-threads=2 &&
    # deposit_replay carries the E-02 FUND-THEFT reproducer and the ten regressions
    # that keep it shut. It is a cargo-auto-discovered target, so for the whole of
    # wave 2 it was named by NO make target and run by nobody: the project's only
    # proven fund-theft primitive had its gate outside the gate. Named explicitly
    # here so that cannot recur silently -- if the file is renamed, this line fails.
    cargo test --test deposit_replay -- --test-threads=2 &&
    # ui_limits reads src/table_canister/src/lib.rs and the two money modals and
    # fails when a limit the UI STATES stops matching the limit the canister
    # ENFORCES (docs/DEFECTS.md T-26: the withdraw modal said 1,000 sats and the
    # canister accepted 11). Two file reads, no replica, so it belongs in the fast
    # gate. Named explicitly for the deposit_replay reason above.
    cargo test --test ui_limits &&
    # deposit_floor is the gate on THE FLOOR INVARIANT
    # (docs/SECURITY-FINDINGS.md FINDING 27, docs/DEFECTS.md E-62: the canister
    # accepted 20,000 e8s at its own advertised minimum and `withdraw` refused
    # anything under 100,000, so a third auditor's deposit could never leave).
    # It drives real deposits and withdrawals through the real canister on
    # PocketIC: the old dead band, the sub-floor residue the pot produces that no
    # equal-floors fix reaches, and the boundary at the ledger fee itself.
    # Named explicitly for the deposit_replay reason above.
    cargo test --test deposit_floor -- --test-threads=2 &&
    # admin_custody is the gate on the ADMIN CUSTODY SURFACE
    # (docs/SECURITY-FINDINGS.md FINDING 07: one controller call destroyed 100% of a
    # funded table's chips, and it survived four waves because nothing in the project
    # ever drove the admin surface with money on the table). It carries the sweep --
    # every controller-callable update, asserting that none of them can reduce what
    # the canister owes players without paying them -- and the census that fails when
    # a NEW controller-gated method appears unaudited. Named explicitly for the
    # deposit_replay reason above.
    cargo test --test admin_custody -- --test-threads=2 &&
    # ledger_boundary is the gate on M14 LEDGER/BOOKS COHERENCE
    # (docs/SECURITY-FINDINGS.md FINDING 29: `deposit`, `claim_external_deposit` and
    # `withdraw` each move real money on the ledger and settle the canister's own
    # books afterwards, in the post-await continuation, so a continuation that does
    # not run leaves the movement standing and the book entry undone). It is the only
    # target that reconstructs a discarded continuation -- snapshot at the await
    # point, let the ledger commit, restore -- and it asserts that the money is
    # accounted for, that the OWNER can recover it with a player-only call, that
    # recovering twice does not credit twice, and that the journal survives an
    # upgrade. Named explicitly for the deposit_replay reason above.
    cargo test --test ledger_boundary -- --test-threads=2 &&
    # coherence_w8 is the gate on THE CURRENCY GUARD'S LAST TERM
    # (docs/SECURITY-FINDINGS.md FINDING 33). `total_liability()` is the ONLY
    # number the guard reads and it has no query, no surface and -- until this
    # target -- no test: the only way to sample it was to attempt the destructive
    # operation it guards. FINDING 33 was one word (`== Pull` under a comment
    # saying `Pull` AND `Sweep`), it made the guard read ZERO on a canister
    # holding 2 ICP of a player's, the resulting currency flip could not be
    # undone, and THREE reviewers drove it in wave 8 without leaving a gate --
    # which is exactly why it survived its own wave. Named explicitly for the
    # deposit_replay reason above.
    cargo test --test coherence_w8 -- --test-threads=2 &&
    # wave6_coherence carries probe1 (the first auditor's fund lock, reached by real
    # silence), probe4 (docs/SECURITY-FINDINGS.md FINDING 17: the fold-out winner
    # must be PAID the pot -- an OUTCOME assertion, because the totals were exact
    # while that defect was live) and probe5 (FINDING 07). All three assert now; the
    # file spent wave 6 outside every target because two of them only RECORDED
    # defects, and a target that passes while the defect is present teaches nobody
    # anything. Named explicitly for the deposit_replay reason above.
    cargo test --test wave6_coherence -- --test-threads=1 &&
    # timers is the gate on THE ON-CHAIN CLOCK (docs/SECURITY-FINDINGS.md FINDING 19,
    # docs/DEFECTS.md E-54/E-55/E-56). Every test in it drives the table with NO
    # ingress message at all after setup -- only subnet ticks and queries -- so it is
    # the only place that can tell whether anything on chain moves the game. It
    # carries: the dead window with every client closed (unbounded before the clock,
    # 30 s after), the post_upgrade re-arm (timers do not survive upgrades, and a
    # canister that silently loses its clock looks fine from outside), the gate that
    # a STALLED hand is played out rather than voided (E-56, which the fuzzer does
    # NOT catch on its own -- reverting that fix leaves `--test fuzz` green), the
    # proof that check_timeouts and the clock reach the same state, and the measured
    # idle-table cycle burn the clock adds. Named explicitly for the deposit_replay
    # reason above.
    cargo test --test timers -- --test-threads=1 &&
    MONEY_FUZZ_SEEDS="${CLEARDECK_SMOKE_FUZZ_SEEDS:-1}" \
    MONEY_FUZZ_STEPS="${CLEARDECK_SMOKE_FUZZ_STEPS:-40}" \
    MONEY_FUZZ_SHRINK=10 \
      cargo test --test fuzz &&
    # AND THE SAME BINARY WITH NOTHING IN THE ENVIRONMENT (docs/DEFECTS.md H-28).
    #
    # The line above is steerable, which is a feature and was also the hole: every
    # caller in this repo passed MONEY_FUZZ_SEEDS, so `seeds()`'s default arm and
    # `DEFAULT_STEPS` were executed by NOTHING and `cargo test --test fuzz` -- the
    # command in the harness's own doc comment -- was red for a whole wave at seed
    # 0xC1EA_2DEC_0003. A smoke run of a configuration nobody ships is not a gate on
    # the configuration everybody types.
    #
    # It costs about 50 s against a gate that already spends 900 s on the settlement
    # oracle, it reuses the wasm and the ledger this step already fetched, and there
    # is deliberately NO opt-out: an environment variable that skips this is how the
    # hole gets dug a second time. `make fuzz-default` runs exactly this alone.
    env -u MONEY_FUZZ_SEEDS -u MONEY_FUZZ_STEPS -u MONEY_FUZZ_SHRINK -u MONEY_FUZZ_REPORT \
      cargo test --test fuzz
  ) || failed+=("money-safety fast subset")

  # THE SETTLEMENT ORACLE IS IN THE DEFAULT GATE ON PURPOSE.
  #
  # It is the ONLY harness in the repo that measures per-seat chip deltas against
  # an independently derived answer, and therefore the only one that convicts a
  # payout that lands the right totals at the WRONG SEAT. Measured by the coherence
  # pass: a mutation that credits 99% of every pot to the winner and 1% to another
  # seated player -- while RECORDING the correct winner and the correct amount --
  # leaves money-safety `invariants` at 30/30 green and `regressions` at 6/6 green,
  # and is caught here. For the whole of wave 2 this suite lived behind its own
  # `make settlement` that no default target and no CI job invoked.
  step "[5/6] settlement oracle (tests/settlement)"
  with_timeout 900 sh -c 'cd tests/settlement && cargo test --test settlement -- --test-threads=1' \
    || failed+=("settlement oracle")
  with_timeout 300 sh -c 'cd tests/settlement && cargo test --test disagreements -- --test-threads=1' \
    || failed+=("settlement pinned reproducers")

  # THE SCREENSHOT HARNESS'S OWN GATES, WHICH NOTHING RAN.
  #
  # `tools/shots/package.json` has carried a `selftest` script since the pixel
  # gate was written and no target invoked it -- in the one harness whose gates
  # cannot run without a replica, so its self-tests are the ONLY part of it a
  # default gate can execute. They need ~20 s and no replica. Among them is the
  # no-rake gate's own failing case (docs/DEFECTS.md E-61).
  step "[6/6] screenshot-harness self-tests (no replica)"
  cmd_shots_selftest || failed+=("screenshot-harness self-tests")

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

# THE FUZZER'S OWN DEFAULT INVOCATION, WITH NOTHING IN THE ENVIRONMENT.
#
# docs/DEFECTS.md H-28. `cd tests/money_safety && cargo test --test fuzz` was RED at
# fe72d46 and had been for a whole wave, and the reason nobody saw it is structural
# rather than careless: EVERY caller passed MONEY_FUZZ_SEEDS and MONEY_FUZZ_STEPS, so
# the code path that reads the defaults -- `seeds()`'s `Err(_) =>` arm and
# `DEFAULT_STEPS` -- was executed by nothing. `make test` passed 1 seed x 40 steps,
# `make fuzz` passed 9 explicit seeds, and neither of them ever landed on seed
# 0xC1EA_2DEC_0003.
#
# `env -u` is the point of this function. Running it under a shell that happens to
# export MONEY_FUZZ_SEEDS would test something else entirely, which is exactly how the
# hole was dug.
cmd_fuzz_default() {
  step "money-safety fuzzer at its OWN DEFAULTS (no environment)"
  info "3 fixed seeds x $DEFAULT_FUZZ_STEPS steps: heads-up, 6-max, 6-max with an ante"
  info "this is the invocation a developer types; docs/DEFECTS.md H-28 is why it has a target"
  cargo build -p table_canister --target wasm32-unknown-unknown --release >/dev/null
  announce_wasm
  (
    cd tests/money_safety
    export CLEARDECK_TABLE_WASM="$WASM_PATH"
    env -u MONEY_FUZZ_SEEDS -u MONEY_FUZZ_STEPS -u MONEY_FUZZ_SHRINK -u MONEY_FUZZ_REPORT \
      cargo test --test fuzz -- --nocapture
  )
}

cmd_fuzz() {
  # The default invocation FIRST, so the long sweep can never again be green while
  # the command in the harness's own doc comment is red.
  cmd_fuzz_default || return 1

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
# cmd: shots-verdict  -- the LAST RECORDED sweep's verdict, as a gate
# ---------------------------------------------------------------------------
#
# docs/DEFECTS.md E-63. `cmd_shots` above already exits 1 on a red scene, and
# that was not enough: it needs a live replica, the replica was unstartable for
# two waves, and a real regression (8.7% of a money figure painted over by the
# felt) therefore sat inside an artifact with NO gate anybody could run being red
# about it. This one reads the tracked verdicts of the last sweep. No replica, no
# browser, no canister -- so a wave that cannot re-run the sweep still has to
# answer for what the sweep last said.
#
# It is also a step inside `hygiene`, because hygiene is the cheap gate every
# wave runs and the whole failure here was reachability.
cmd_shots_verdict() {
  step "the last recorded screenshot verdict"
  node tools/shots/verdict-gate.mjs
}

# ---------------------------------------------------------------------------
# cmd: shots-selftest  -- the screenshot harness's gates, gating themselves
# ---------------------------------------------------------------------------
#
# THESE EXISTED AND NOTHING RAN THEM. `tools/shots/package.json` has carried a
# `selftest` script since the pixel gate was written, and no make target, no
# dev.sh command and no CI job invoked it -- the same hole as `deposit_replay` in
# wave 2 and `--test fuzz` in H-28, in the one harness whose gates cannot run
# without a replica. They need no replica: every case is a stub page or a fixture
# whose answer is known by construction.
cmd_shots_selftest() {
  step "screenshot-harness self-tests (no replica)"
  if [ ! -d tools/shots/node_modules/playwright ]; then
    info "installing playwright in tools/shots"
    ( cd tools/shots && npm install --no-audit --no-fund )
  fi
  local failed=()
  # the pixel gate, on overlaps whose answer is written into the fixture
  node tools/shots/test-occlusion.mjs      || failed+=("test-occlusion")
  # the inverted money gate: a token nothing asserts must fail a scene
  node tools/shots/test-census.mjs         || failed+=("test-census")
  # the display-vs-e8s parser
  node tools/shots/test-money.mjs          || failed+=("test-money")
  # THE NO-RAKE GATE (docs/DEFECTS.md E-61): it must go red on a rake of 1 e8,
  # and a missing field must be a structural failure and never a silent NaN
  node tools/shots/test-rake.mjs           || failed+=("test-rake")
  # the action dock's containment (docs/DEFECTS.md E-63), measured against
  # PokerTable.svelte's own stylesheet
  node tools/shots/test-dock-overflow.mjs  || failed+=("test-dock-overflow")
  if [ ${#failed[@]} -eq 0 ]; then ok "all screenshot-harness self-tests green"; return 0; fi
  warn "FAILED: ${failed[*]}"
  return 1
}

# ---------------------------------------------------------------------------
# cmd: known-defects  -- the markers that are RED on purpose
# ---------------------------------------------------------------------------
#
# Five tests in tools/differential are #[ignore]d and written to FAIL until the
# engine is fixed. They are NOT part of `test`, because a suite that is red by
# design teaches everyone to ignore red. This target inverts them: it succeeds
# while they still fail, and tells you the moment one goes green.
#
# This list covers the EVALUATOR defects only. The payout defects (docs/DEFECTS.md
# E-01, E-03, E-05, E-35) were fixed in wave 2, so their markers were inverted into
# gates and live where the suite already runs them, not here:
#   `test`        reg01/reg05/reg08/reg09 + seam_a + m1b (tests/money_safety), and
#                 payout_tests (src/table_canister/src/lib.rs)
#   `settlement`  pinned_e01 / pinned_e05 / pinned_odd_chips + a per-hand gate on
#                 every hand the oracle drives

# Four of the original five went GREEN when E-09 (poker_core input validation) was
# fixed in wave 2. A marker that has gone green and STAYS in this list is worse than
# no marker: the target shouts every run and everybody learns to ignore it. So they
# were un-#[ignore]d in tools/differential/tests/fast_subset.rs -- where
# `./scripts/dev.sh test` runs them as ordinary gates -- and removed from here.
# That is the whole lifecycle: red-on-purpose marker -> defect fixed -> gate.
#
# What is left is E-13, which is genuinely still present.
DEFECT_MARKERS=(
  defect_detect_straight_returns_the_best_straight
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
  # WHAT THIS COUNTS, AND WHAT IT DELIBERATELY DOES NOT.
  #
  # The question is "how many bytes would a commit ADD", so a staged DELETION is
  # excluded even though the file is still on disk. Removing the 23 MB of tracked
  # screenshot manifests (docs/DEFECTS.md E-60) is the change that showed this up:
  # it reduces the repo, and the old counter billed it as +23 MB and failed.
  local n bytes
  local -a candidates=()
  while IFS= read -r line; do
    [ -z "$line" ] && continue
    case "$line" in
      'D '*|' D'*|'DD'*) continue ;;   # a deletion adds nothing
    esac
    candidates+=("${line:3}")
  done < <(git status --porcelain --untracked-files=all)
  n="${#candidates[@]}"
  bytes=0
  local f sz
  for f in "${candidates[@]}"; do
    [ -f "$f" ] || continue
    sz="$(wc -c <"$f" | tr -d ' ')"
    bytes=$((bytes + sz))
  done
  info "$n added/modified path(s), $((bytes/1024)) KiB total"
  if printf '%s\n' "${candidates[@]}" \
       | grep -Eiq '\.(png|jpg|jpeg|gif|webp|webm|mp4|wasm|gz|zip|bin|pdf|tiff|bmp)$'; then
    warn "binary/media files are staged for commit:"
    printf '%s\n' "${candidates[@]}" \
      | grep -Ei '\.(png|jpg|jpeg|gif|webp|webm|mp4|wasm|gz|zip|bin|pdf|tiff|bmp)$' | sed 's/^/      /'
    bad=1
  else
    ok "no binary or image files would be committed"
  fi
  if [ "$bytes" -gt $((4 * 1024 * 1024)) ]; then
    warn "added payload exceeds 4 MiB; check for a stray artifact directory"
    bad=1
  fi

  # THE CHECK THAT WOULD HAVE CAUGHT E-60 A WAVE EARLIER.
  #
  # The counter above only ever looked at UNTRACKED payload, so committing a
  # 7.6 MB machine-generated file made hygiene go GREEN while making the problem
  # permanent -- and the file then grew 10x in a single wave with nothing
  # watching. Evidence under artifacts/ is meant to be small and readable: the
  # verdicts, not the working-out. Anything big enough to hide in is build output
  # and belongs beside the PNGs, not in a public history.
  step "no tracked artifact is a machine-generated blob"
  local cap=$((512 * 1024)) oversize=0
  while IFS= read -r f; do
    [ -f "$f" ] || continue
    sz="$(wc -c <"$f" | tr -d ' ')"
    if [ "$sz" -gt "$cap" ]; then
      warn "$f is $((sz/1024)) KiB, over the $((cap/1024)) KiB cap for tracked artifacts"
      oversize=1
    fi
  done < <(git ls-files -- artifacts)
  if [ "$oversize" = "0" ]; then
    ok "every tracked file under artifacts/ is under $((cap/1024)) KiB"
  else
    info "if it is a run's working-out, gitignore it and keep the verdicts (docs/DEFECTS.md E-60)"
    bad=1
  fi

  # THE LAST RECORDED SCREENSHOT VERDICT, AS A GATE.
  #
  # docs/DEFECTS.md E-63: the pixel gate caught a real regression and nothing
  # anybody could RUN was red about it, because the only instrument that sees a
  # rendered failure needs a live replica and the replica was down for two waves.
  # This reads the tracked verdicts of the last sweep, needs nothing, and stays
  # red until every red is written down -- and until every acknowledgement that
  # is no longer red is deleted.
  step "every recorded screenshot red is acknowledged"
  if node tools/shots/verdict-gate.mjs; then :; else bad=1; fi

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
# cmd: settlement   (appended -- owner: tests/settlement/**)
# ---------------------------------------------------------------------------
#
# The independent settlement oracle. Derives what each seat is OWED from the rules
# of poker and compares that against what the real canister actually paid, hand by
# hand, on PocketIC.
#
# It builds and identifies its own wasm (tests/settlement/src/wasms.rs) rather than
# trusting $WASM_PATH, and refuses to run against an artifact older than its
# sources, so it is deliberately NOT wired through wasm_env/announce_wasm.

cmd_settlement() {
  local scope="${1:-all}"

  step "settlement oracle: its own rules (no replica)"
  ( cd tests/settlement && cargo test --test oracle_rules ) || return 1

  step "settlement oracle: golden reproducers of every disagreement (no replica)"
  ( cd tests/settlement && cargo test --test disagreements -- golden ) || return 1

  if [ "$scope" = "fast" ]; then
    ok "fast settlement subset passed (skipped the PocketIC comparison runs)"
    return 0
  fi

  step "settlement oracle: the comparison harness vs the REAL canister"
  ( cd tests/settlement && cargo test --test settlement -- --test-threads=1 --nocapture ) || return 1

  step "settlement oracle: the payout defects it found, pinned as FIXED"
  ( cd tests/settlement && cargo test --test disagreements -- pinned --test-threads=1 --nocapture ) || return 1

  ok "settlement oracle run complete"
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
                    --docker       install the reproducible container build instead
                                   of your own toolchain's, so that
                                   'verify-build.sh --local' runs the same check
                                   a stranger runs against mainnet
  ${B}local-status${R}    alias for doctor
  ${B}wasm${R}            build table_canister.wasm and print its sha256

  ${B}test${R}            FAST gate: workspace tests + wasm build + differential fast
                  subset + money-safety invariants/regressions/ui_limits + a short
                  fuzz run AND the fuzzer at its own defaults (docs/DEFECTS.md H-28).
                  No replica needed.
  ${B}fuzz-default${R}    the fuzzer exactly as a developer runs it: no environment at
                  all, so the DEFAULT seeds and DEFAULT step count are what runs.
                  docs/DEFECTS.md H-28 -- this invocation was red for a whole wave
                  because every other target overrode it.
  ${B}fuzz${R}            LONG: fuzz-default first, then 9 seeds x 600 hostile steps
                  against the real canister and the real ICP ledger on PocketIC
  ${B}settlement${R}      the independent settlement oracle: derives what each seat is
                  OWED from the rules of poker and compares it against what the
                  real canister paid, hand by hand. 'settlement fast' skips the
                  PocketIC runs and keeps the rules + golden reproducers.
  ${B}diff-full${R}       exhaustive evaluator differential (all C(52,5) x 3 evaluators)
  ${B}shots${R}           screenshot the real UI against the real local canisters
                  (requires local-up)
  ${B}shots-verdict${R}   the LAST RECORDED sweep's verdict, as a gate. Reads the tracked
                  artifacts/screens/latest/verdicts.json and fails on any red that
                  is not written down in acknowledged-reds.json under a filed
                  defect -- and on any acknowledgement that is no longer red.
                  No replica: this is how a wave that CANNOT run the sweep still
                  has to answer for what it last said (docs/DEFECTS.md E-63).
  ${B}shots-selftest${R}  the screenshot harness's own gates, on fixtures whose answer is
                  known by construction: the pixel gate, the token census, the
                  money parser, the NO-RAKE gate and the action dock's
                  containment. No replica.
  ${B}known-defects${R}   run the markers that are RED on purpose; succeeds while the
                  engine defects are still present, shouts when one is fixed
  ${B}hygiene${R}         no large/binary files added, no tracked artifact blob, every
                  recorded screenshot red acknowledged; disclaimer, 18+,
                  jurisdiction and no-rake notices present and not weakened
                  since ${BASELINE_COMMIT}
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
    fuzz-default)   cmd_fuzz_default ;;
    settlement)     cmd_settlement "$@" ;;
    diff-full)      cmd_diff_full "$@" ;;
    shots)          cmd_shots "$@" ;;
    shots-verdict)  cmd_shots_verdict ;;
    shots-selftest) cmd_shots_selftest ;;
    known-defects)  cmd_known_defects ;;
    hygiene)        cmd_hygiene ;;
    selftest)       cmd_selftest ;;
    phe-venv)       cmd_phe_venv ;;
    *) die "unknown command '$cmd' (try '$0 help')" ;;
  esac
}

main "$@"
