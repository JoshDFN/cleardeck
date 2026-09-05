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
  # POSTCONDITION, READ BACK, AND FATAL. docs/DEFECTS.md E-74.
  #
  # The two calls above are `|| warn`, and `icp canister call` exits 0 on a
  # `variant { Err }` (docs/DEFECTS.md E-50), so BOTH can fail and this function
  # still reported success. It did: on 2026-08-06 `local-up` printed
  # `✓ lobby lists 0 table record(s)` and exited 0, and the ENTIRE screenshot
  # harness -- the only gate in this project that measures the four protected
  # notices on rendered pixels -- failed all 24 scenes with "Lobby has no
  # registered name for table_N". A gate that cannot run is a gate that is off,
  # and this one was off silently.
  #
  # Root cause when it happens: `set_admin` above is called AS $CONTROLLER, but
  # the lobby's admin is whoever initialised it first. If that was a different
  # identity, `set_admin` is refused ("Only current admin can set new admin") and
  # `init_microstakes_tables` is then refused ("Only admin can initialize
  # tables"). Recover by handing the admin over from the identity that holds it:
  #
  #   icp canister call lobby get_admin '()' --query -e local
  #   icp canister call lobby set_admin "(principal \"<$CONTROLLER's principal>\")" \
  #     --identity <the identity get_admin named> -e local
  #
  # then run this again.
  if [ "${count:-0}" -lt 1 ]; then
    icp_local canister call lobby get_admin '()' --query 2>/dev/null \
      | sed 's/^/      lobby admin: /' >&2 || true
    die "the lobby lists NO tables. local-up used to report success here; it does not any \
more, because an empty lobby makes the whole screenshot harness unrunnable and nothing else \
notices. See the recovery in up_wire (docs/DEFECTS.md E-74)."
  fi
  ok "lobby lists $count table record(s)"
  # THE RECORDS SAY WHAT THE CONTRACTS CHARGE. docs/DEFECTS.md T-11.
  # `init_microstakes_tables` writes table_1's blinds and buy-ins into all three
  # records, while table_2 and table_3 enforce 5x and 10x those figures. The
  # lobby has a read-only repair for exactly that: `refresh_all_table_configs`
  # copies each registered table's config FROM the table canister itself, so
  # the registry cannot be told a figure the contract does not charge. Without
  # it the lobby scene fails chain agreement on a freshly provisioned stack
  # (every local-up since E-74), which is the harness working, not the client.
  lobby_sync_records
  # btc_table_1 has no lobby registration call in any deploy path. State it rather
  # than let it look intentional. docs/DEFECTS.md T-05.
  warn "btc_table_1 is NOT registered in the lobby by any known call (docs/DEFECTS.md T-05)"
}

# Copies every registered table's config from the table contract into the lobby
# record (admin only; $CONTROLLER is the admin local-up sets). Read back and
# fatal, like the count above: an `Err` reply exits 0 from `icp canister call`.
lobby_sync_records() {
  local reply
  reply="$(icp_local canister call lobby refresh_all_table_configs '()' --identity "$CONTROLLER" 2>&1 || true)"
  if ! printf '%s' "$reply" | grep -q 'Ok'; then
    printf '%s\n' "$reply" | sed 's/^/      /' >&2
    die "lobby refresh_all_table_configs did not reply Ok; the lobby records still carry \
init_microstakes_tables' figures (docs/DEFECTS.md T-11) and the lobby scene will fail chain agreement"
  fi
  ok "lobby records refreshed from the table contracts: $(printf '%s' "$reply" | tr -d '\n' | sed 's/  */ /g')"
}

cmd_local_lobby_sync() {
  gateway_is_up || die "the local gateway is not answering at ${GATEWAY_ORIGIN}; run '$0 local-up'"
  lobby_sync_records
  icp_local canister call lobby get_tables '()' --query 2>/dev/null \
    | grep -E 'name|small_blind|big_blind|min_buy_in|max_buy_in' | sed 's/^/      /'
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
# THE WATCHDOG USED TO OUTLIVE THE RUN, AND THAT IS docs/DEFECTS.md H-42 (H-54).
#
# `kill "$wd"` kills the SUBSHELL. The `sleep` it forked is a separate process and
# survives, orphaned to init, still holding the stdout and stderr it inherited. So
# `./scripts/dev.sh test` would EXIT NORMALLY and any consumer of its output --
# `| tail`, `| tee`, `$( )`, a CI log collector -- would then block for up to the
# remaining 900 seconds with zero CPU anywhere, because the pipe's write end was
# still open. Measured: `dev.sh test` finished, and `sleep 900` sat at PPID 1
# holding the pipe for another fourteen minutes. H-42 recorded the symptom ("hung
# for 33 minutes with zero CPU on both the test binary and its own PocketIC") and
# blamed the step BEFORE this one for having no time bound; the cause is the two
# steps that DO have one. 900 + 300 is the 20 minutes, and it is entirely
# post-hoc: every gate had already passed or failed.
#
# Two independent fixes, because either alone would do and both are one line:
#   * the watchdog's own output goes to /dev/null, so a leak cannot hold the pipe;
#   * its children are killed before it is, so there is nothing left to leak.
#
# AND THAT FIXED THE WATCHDOG'S LEAK WHILE LEAVING THE BOUND ITSELF VACUOUS
# (docs/DEFECTS.md H-42, wave 14). H-54 killed the watchdog's children and never
# killed the BOUNDED COMMAND'S children. `kill -9 "$pid"` reaches exactly one
# process: the subshell, which bash has usually exec'd into `sh`. Everything that
# `sh` forked -- `cargo`, the test binary, its PocketIC server -- is orphaned to
# init and KEEPS THE INHERITED STDOUT PIPE OPEN, so a consumer still blocks for as
# long as the runaway lives. Measured on this file at HEAD, before the change
# below:
#
#     out="$(with_timeout 2 sh -c 'sleep 60; echo never')"
#     -> command substitution returned after 60s
#
# A two-second bound that returns in sixty seconds is not a bound, and "the
# consumer blocks with zero CPU anywhere" is H-42's symptom word for word. So the
# timeout now kills the whole DESCENDANT TREE, deepest first, and does it twice:
# once when the bound expires and once after the wait, because a runaway can fork
# between the survey and the kill.

# Every descendant of $1, deepest first. `ps` is the only portable process table
# on macOS and Linux both; `pkill -P` is one level only, which is what made the
# old form vacuous.
descendants() {
  local root="$1" kid
  for kid in $(ps -eo pid=,ppid= | awk -v p="$root" '$2 == p { print $1 }'); do
    descendants "$kid"
    printf '%s\n' "$kid"
  done
}

# Kill $1 and everything below it. Children FIRST: killing a parent first is what
# orphans its children onto init still holding the pipe.
kill_tree() {
  local root="$1" sig="${2:-KILL}" p
  for p in $(descendants "$root"); do kill -"$sig" "$p" 2>/dev/null || true; done
  kill -"$sig" "$root" 2>/dev/null || true
}

with_timeout() {
  local secs="$1"; shift
  local t0 elapsed
  t0="$(date +%s)"
  ( "$@" ) & local pid=$!
  ( sleep "$secs"; kill_tree "$pid" ) >/dev/null 2>&1 & local wd=$!
  wait "$pid"; local rc=$?
  # A SECOND SWEEP ONLY WHEN THE WATCHDOG ACTUALLY FIRED.
  #
  # A runaway can fork between the watchdog's `ps` survey and its kill, so one
  # pass is not enough. But `$pid` has been reaped by `wait`, and a pid is
  # reusable the moment it is reaped -- sweeping it unconditionally is a small
  # chance of killing a stranger's process tree every time a step passes, which
  # is not a trade a test runner gets to make. So: only after a kill, and only
  # if something still answers to that pid.
  if [ "$rc" -ge 128 ] && kill -0 "$pid" 2>/dev/null; then
    kill_tree "$pid" >/dev/null 2>&1 || true
  fi
  # The watchdog is normally still alive here, sleeping out the rest of the
  # bound. Children first: killing the subshell first is what orphans the `sleep`
  # onto init with the pipe still open, which is docs/DEFECTS.md H-54.
  if kill -0 "$wd" 2>/dev/null; then
    kill_tree "$wd" >/dev/null 2>&1 || true
  fi
  wait "$wd" 2>/dev/null || true
  elapsed=$(( $(date +%s) - t0 ))
  if [ "$rc" -ge 128 ]; then
    printf '    %sTIMED OUT%s after %ss (bound %ss): %s\n' "$E" "$R" "$elapsed" "$secs" "$*" >&2
    # A killed command must never look like a pass, whatever signal did it.
    return 124
  fi
  return "$rc"
}

# `with_timeout` plus a one-line "how long did that actually take", because H-42
# could name the wall-clock total and NOT the step, which is what turned a
# reproducible hang into a wave of guessing.
timed_step() {
  local secs="$1" label="$2"; shift 2
  local t0 rc
  t0="$(date +%s)"
  with_timeout "$secs" "$@"; rc=$?
  printf '    %s took %ss (bound %ss)\n' "$label" "$(( $(date +%s) - t0 ))" "$secs"
  return "$rc"
}

# EVERY STEP OF THE PRIMARY GATE HAS A TIME BOUND, AND THE BOUNDS ARE HERE
# (docs/DEFECTS.md H-42).
#
# H-42 is `./scripts/dev.sh test` hanging for 33 minutes with zero CPU on both the
# test binary and its own PocketIC, and the entry's own diagnosis was "the
# money-safety targets have NO time bound". They have one now, and so does every
# other step, because "which step is it in" was the question nobody could answer.
#
# Each bound is roughly 2.5x the measured wall clock of that step on this machine,
# recorded next to it. The multiple is deliberately large: a bound that trips on a
# slow laptop teaches people to raise bounds, which is how a bound stops meaning
# anything. What it has to catch is a step that has stopped making progress at all,
# and every one of those in this register has been a hang, not a slowdown.
# Measured end to end on this machine on 2026-08-09, in the run that turned this
# gate green (total 1894 s, about 32 minutes):
BOUND_WORKSPACE=900          # measured 24 s
BOUND_WASM=900               # measured 0 s warm; a cold build is ~180 s
BOUND_DIFFERENTIAL=900       # measured 3 s
BOUND_MONEY_SAFETY=3600      # measured 1610 s -- 22 cargo targets, the long pole
BOUND_SETTLEMENT=900         # measured 101 s
BOUND_SETTLEMENT_PINNED=300  # measured 13 s
BOUND_SHOTS_SELFTEST=600     # measured 4 s
BOUND_ARCHIVE=600            # measured 88 s
BOUND_NO_PEEKING=900         # measured 42 s

# The money-safety fast subset, as its own function so `with_timeout` can bound it.
# A bound cannot be put around a bare `( ... )` block, and putting the block behind
# a name is also what lets the step be run on its own while debugging.
#
# EVERY TARGET RUNS, EVEN AFTER ONE OF THEM GOES RED.
#
# This block used to be a single `&&` list, and that is docs/DEFECTS.md H-45
# wearing its third face: `cargo test --test deposit_floor && cargo test --test
# admin_custody && ...` means ONE red target SKIPS THE FIFTEEN AFTER IT. Measured
# on this tree during wave 14: a single failing assertion in `deposit_floor`
# (target 5 of 18) ended the step in 131 seconds having never run `solvency`,
# `stall_agreement`, `fund_reachability`, `controller_custody` or either fuzz
# invocation -- so a wave could fix the deposit-floor red, see the step go green,
# and never learn that a money invariant had been silently skipped in between.
# "Run by nothing" and "skipped because something before it failed" are the same
# hole; only the second one has a green tick further up the log.
#
# So each target is invoked through `run_ms`, which records a failure and carries
# on, and the function reports the whole list at the end.
run_ms() {
  local target="$1"; shift
  cargo test --test "$target" "$@" || ms_failed+=("$target")
}

money_safety_fast_subset() {
  announce_wasm
  (
    cd tests/money_safety
    export CLEARDECK_TABLE_WASM="$WASM_PATH"
    # Collected, not short-circuited. See the comment above run_ms.
    local ms_failed=()
    run_ms invariants   -- --test-threads=2
    run_ms regressions  -- --test-threads=2
    # deposit_replay carries the E-02 FUND-THEFT reproducer and the ten regressions
    # that keep it shut. It is a cargo-auto-discovered target, so for the whole of
    # wave 2 it was named by NO make target and run by nobody: the project's only
    # proven fund-theft primitive had its gate outside the gate. Named explicitly
    # here so that cannot recur silently -- if the file is renamed, this line fails.
    run_ms deposit_replay  -- --test-threads=2
    # ui_limits reads src/table_canister/src/lib.rs and the two money modals and
    # fails when a limit the UI STATES stops matching the limit the canister
    # ENFORCES (docs/DEFECTS.md T-26: the withdraw modal said 1,000 sats and the
    # canister accepted 11). Two file reads, no replica, so it belongs in the fast
    # gate. Named explicitly for the deposit_replay reason above.
    run_ms ui_limits
    # deposit_floor is the gate on THE FLOOR INVARIANT
    # (docs/SECURITY-FINDINGS.md FINDING 27, docs/DEFECTS.md E-62: the canister
    # accepted 20,000 e8s at its own advertised minimum and `withdraw` refused
    # anything under 100,000, so a third auditor's deposit could never leave).
    # It drives real deposits and withdrawals through the real canister on
    # PocketIC: the old dead band, the sub-floor residue the pot produces that no
    # equal-floors fix reaches, and the boundary at the ledger fee itself.
    # Named explicitly for the deposit_replay reason above.
    run_ms deposit_floor  -- --test-threads=2
    # admin_custody is the gate on the ADMIN CUSTODY SURFACE
    # (docs/SECURITY-FINDINGS.md FINDING 07: one controller call destroyed 100% of a
    # funded table's chips, and it survived four waves because nothing in the project
    # ever drove the admin surface with money on the table). It carries the sweep --
    # every controller-callable update, asserting that none of them can reduce what
    # the canister owes players without paying them -- and the census that fails when
    # a NEW controller-gated method appears unaudited. Named explicitly for the
    # deposit_replay reason above.
    run_ms admin_custody  -- --test-threads=2
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
    run_ms ledger_boundary  -- --test-threads=2
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
    run_ms coherence_w8  -- --test-threads=2
    # deposit_surface is the gate on THE ONE DEPOSIT ADDRESS
    # (docs/SECURITY-FINDINGS.md FINDING 06, 11, 34, 39). `get_deposit_address()`
    # published the canister's MAIN account -- the same 64 characters to every
    # player -- under the name "deposit address", and an auditor's 1 ICP arrived
    # where no surface could attribute it to anybody. This target is the only one
    # that spends through the LEGACY `transfer` endpoint, which is the only door a
    # 64-hex address can be paid through and the door every existing test assumed
    # instead of executing; it also runs the FRONTEND's own derivation in node and
    # compares it with the canister, principal by principal, so a client that
    # derives its address locally (which is what removes the uncertified query from
    # the trust path) cannot silently derive a WRONG one. Named explicitly for the
    # deposit_replay reason above.
    run_ms deposit_surface  -- --test-threads=2
    # deposit_trust_root is the gate on WHERE THE CANISTER ID CAME FROM
    # (docs/SECURITY-FINDINGS.md FINDING 42). deposit_surface above proves the
    # derivation's ARITHMETIC agrees with the canister, and stays green while the
    # id being hashed is one a lobby QUERY supplied -- substitute it and every
    # derived address moves into a canister the attacker controls, with the
    # modal's cross-check still passing because it asks that same canister. Six
    # file reads and two node runs, no replica, so it belongs in the fast gate.
    run_ms deposit_trust_root
    # deposit_subaccount_anchor is the gate on THE ACCOUNT CENSUS
    # (docs/SECURITY-FINDINGS.md FINDING 28, FINDING 21, FINDING 11). Eleven tests,
    # one per re-anchoring, each proved to go red on its own revert. It was
    # cargo-auto-discovered and named by NOTHING, so FINDING 11's only gate --
    # "dust is visible and recoverable by topping up" -- was outside every target
    # that anyone runs. Named explicitly for the deposit_replay reason above.
    run_ms deposit_subaccount_anchor  -- --test-threads=2
    # wave6_coherence carries probe1 (the first auditor's fund lock, reached by real
    # silence), probe4 (docs/SECURITY-FINDINGS.md FINDING 17: the fold-out winner
    # must be PAID the pot -- an OUTCOME assertion, because the totals were exact
    # while that defect was live) and probe5 (FINDING 07). All three assert now; the
    # file spent wave 6 outside every target because two of them only RECORDED
    # defects, and a target that passes while the defect is present teaches nobody
    # anything. Named explicitly for the deposit_replay reason above.
    run_ms wave6_coherence  -- --test-threads=1
    # oldest_cluster is the gate on THE OLDEST CLUSTER IN THE REGISTER
    # (docs/SECURITY-FINDINGS.md FINDING 02, 05, 08, 09, 17 and 22). Five of the six
    # were closed in waves 2, 7 and 8 and their headers never said so; this target is
    # what turns each of those closures from a blockquote into something that goes
    # red when it stops being true, and it carries FINDING 22 -- the recovery door
    # that could void a live hand, conserving to the e8, which no invariant in this
    # project could see. Named explicitly for the deposit_replay reason above, and
    # wired in the same change that files the fix: a FIXED row whose gate no target
    # runs is docs/DEFECTS.md H-45, which this wave is trying to shrink, not grow.
    run_ms oldest_cluster  -- --test-threads=2
    # fund_reachability, solvency and stall_agreement were cargo-auto-discovered and
    # named by NOTHING -- 20 tests run by no target, no make rule and no CI job.
    # That is docs/DEFECTS.md H-45, and it is not academic: fund_reachability is the
    # gate on M9, the property added after an auditor locked ~420 ICP, and it sat RED
    # for a whole wave without anyone seeing it, because the tests it holds were
    # measuring the pre-on-chain-clock semantics. A gate nothing runs does not decay
    # into a useless gate, it decays into a MISLEADING one: the register cites it as
    # what holds a finding closed.
    run_ms fund_reachability  -- --test-threads=1
    run_ms solvency  -- --test-threads=2
    run_ms stall_agreement  -- --test-threads=1
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
    run_ms timers  -- --test-threads=1
    # cycles_runway is the gate on THE RUNWAY (docs/DEFECTS.md E-55,
    # docs/SECURITY-FINDINGS.md FINDING 24/26). `timers` measures the IDLE burn, and
    # idle is the number for a table nobody is using. THIS SUITE'S OWN "with N tabs
    # open" ROWS ARE STILL WRONG (docs/DEFECTS.md E-97): they price a tab as a
    # heartbeat stream only. The measured cost of an open tab, and the runway table
    # every other consumer reads, is tools/cycles/burn-table.json. It also carries
    # the port of FINDING 24's
    # probe -- a frozen canister rejects QUERIES too, which the finding says is the
    # load-bearing sentence of the whole cycles plan and which nothing in this tree
    # executed -- and the gate on `get_cycle_status` itself: a lifetime burn average
    # reads HIGH on a table that has just got busy, and a fuel gauge that reads high
    # is the failure that matters. Named explicitly for the deposit_replay reason
    # above. ~4 min.
    run_ms cycles_runway  -- --test-threads=1
    # controller_custody is the gate on THE CONTROLLER SEAT
    # (docs/SECURITY-FINDINGS.md FINDING 23). One controller principal per canister,
    # and `install_code --mode reinstall` or `uninstall_code` on a funded table
    # destroys every balance while the ledger keeps the ICP -- the fifth auditor
    # executed it for 5 ICP of her own. NOTHING in this project could see it and
    # nothing could see it by construction: admin_custody's subject is methods that
    # call `require_controller()` INSIDE the canister, and these two are calls to the
    # MANAGEMENT canister. Two halves: `finding23_*` PIN the defect with numbers
    # (they pass while it is live, which is the point -- an unmeasured critical is a
    # forgotten critical), and `guardian_*` measure the fix and, first, its PREMISE,
    # that controllership is not transitive. Named explicitly for the deposit_replay
    # reason above; wired in the same change that files the guardian, because a
    # mitigation whose gate no target runs is docs/DEFECTS.md H-45.
    run_ms controller_custody  -- --test-threads=2
    env MONEY_FUZZ_SEEDS="${CLEARDECK_SMOKE_FUZZ_SEEDS:-1}" \
        MONEY_FUZZ_STEPS="${CLEARDECK_SMOKE_FUZZ_STEPS:-40}" \
        MONEY_FUZZ_SHRINK=10 \
      cargo test --test fuzz || ms_failed+=("fuzz (smoke seeds)")
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
      cargo test --test fuzz || ms_failed+=("fuzz (own defaults)")
    # solvency_definition is the gate on ONE DEFINITION OF HELD AND OWED
    # (docs/SECURITY-FINDINGS.md FINDING 43 and FINDING 38). It is here, LAST in the
    # chain, for a reason worth stating: it was written during wave 14 by another
    # owner and named by nothing, which is docs/DEFECTS.md H-45 for the SEVENTH
    # time -- and this time step [1/9] said so within a second instead of a human
    # finding it a wave later. Last in the chain because this is an `&&` list: a
    # target that fails skips every target after it, so the newest arrival goes
    # where it can hide nothing.
    run_ms solvency_definition  -- --test-threads=2

    if [ ${#ms_failed[@]} -eq 0 ]; then
      ok "money-safety: all $MS_TARGET_COUNT cargo targets green"
      exit 0
    fi
    printf '    %sMONEY-SAFETY TARGETS FAILED (%d of %d):%s %s\n' \
      "$E" "${#ms_failed[@]}" "$MS_TARGET_COUNT" "$R" "${ms_failed[*]}" >&2
    info "every other target above still RAN; this list is COMPLETE, not first-failure"
    exit 1
  )
}

# How many `run_ms` invocations the block above makes, computed from the file so
# it cannot drift from the list. Printed in the summary so "all targets green" is
# a countable claim rather than an adjective.
MS_TARGET_COUNT="$(( $(grep -cE '^[[:space:]]+run_ms ' "${BASH_SOURCE[0]}") + 2 ))"

cmd_test() {
  local failed=()

  # THE CHEAPEST GATE IN THE FILE, AND IT GOES FIRST (docs/DEFECTS.md H-45).
  #
  # H-45 is "six money-safety suites, 31 tests, run by no target", and it is H-17,
  # H-50 and H-53 wearing the same clothes: the fourth, fifth and sixth time a test
  # file was added to this repository and named by nothing. Every one of those was
  # found by a human reading a directory listing a wave later.
  #
  # `check-suite-wiring.sh` is that reading, mechanised: it lists every cargo test
  # target on disk, requires each to appear in scripts/test-suites.list, and
  # requires every row there to name a tier something actually runs. It has been in
  # CI since wave 13 -- and CI is not what a developer runs before pushing, and by
  # this project's own reckoning nothing in CI is a required check anyway (H-23).
  # It costs under a second and it is the only step here that can catch a suite
  # NOBODY RUNS, so it runs before the suites do, and a failure is fatal rather
  # than collected: there is no point measuring coverage with a list you know is
  # wrong.
  step "[1/9] the gates' own wiring (docs/DEFECTS.md H-45, D-11)"
  timed_step 120 "suite wiring" ./scripts/check-suite-wiring.sh \
    || die "suite wiring is broken -- a test target in this tree is run by nothing. Fix scripts/test-suites.list before trusting anything below."
  # AND THE THIRD COPY OF EVERY INTERFACE (docs/DEFECTS.md D-11).
  #
  # `src/declarations/<n>/<n>.did.js` is what the app builds its actors from, and
  # it is neither the Rust nor the committed `.did`. It was missing sixteen
  # methods on the table binding alone -- among them `get_solvency` and
  # `refresh_solvency`, so the custody instruments this gate spends twenty minutes
  # exercising could not be called from the product at all. Regenerate-and-diff,
  # seconds, no replica; it is here rather than in a wasm-building step because a
  # binding that cannot reach a method is a defect in the shipped app, not in the
  # build.
  timed_step 300 "candid bindings" ./scripts/check-declarations-js.sh \
    || failed+=("frontend Candid bindings (./scripts/check-declarations-js.sh --write)")

  step "[2/9] cargo test --workspace"
  timed_step "$BOUND_WORKSPACE" "workspace" cargo test --workspace \
    || failed+=("cargo test --workspace")

  step "[3/9] table_canister wasm build"
  timed_step "$BOUND_WASM" "wasm build" cmd_wasm || failed+=("wasm build")

  step "[4/9] differential fast subset (tools/differential)"
  timed_step "$BOUND_DIFFERENTIAL" "differential" \
    sh -c 'cd tools/differential && cargo test' || failed+=("differential fast subset")

  step "[5/9] money-safety fast subset + the fuzzer at its own defaults"
  timed_step "$BOUND_MONEY_SAFETY" "money-safety fast subset" money_safety_fast_subset \
    || failed+=("money-safety fast subset")

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
  step "[6/9] settlement oracle (tests/settlement)"
  timed_step "$BOUND_SETTLEMENT" "settlement oracle" \
    sh -c 'cd tests/settlement && cargo test --test settlement -- --test-threads=1' \
    || failed+=("settlement oracle")
  timed_step "$BOUND_SETTLEMENT_PINNED" "settlement pinned reproducers" \
    sh -c 'cd tests/settlement && cargo test --test disagreements -- --test-threads=1' \
    || failed+=("settlement pinned reproducers")

  # THE SCREENSHOT HARNESS'S OWN GATES, WHICH NOTHING RAN.
  #
  # `tools/shots/package.json` has carried a `selftest` script since the pixel
  # gate was written and no target invoked it -- in the one harness whose gates
  # cannot run without a replica, so its self-tests are the ONLY part of it a
  # default gate can execute. They need ~20 s and no replica. Among them is the
  # no-rake gate's own failing case (docs/DEFECTS.md E-61).
  step "[7/9] screenshot-harness self-tests (no replica)"
  timed_step "$BOUND_SHOTS_SELFTEST" "shots self-tests" cmd_shots_selftest \
    || failed+=("screenshot-harness self-tests")

  # THE ARCHIVE ANALYSER'S 39 GATES, WHICH NOTHING RAN (docs/DEFECTS.md H-50).
  #
  # `tools/archive/**` shipped in wave 12 with 39 offline self-tests and NO dev.sh
  # target, no make rule and no CI job -- stated as held by them in the very section
  # of docs/ARCHIVE.md that opens "A gate nothing runs is worse than no gate". They
  # need no replica and no network: every population's truth is set by construction.
  # ~85 s.
  #
  # It is in the DEFAULT gate rather than only in its own target because the tool
  # reconstructs hole cards from the published seed and prices folds in ICP: it is a
  # second, independent implementation of docs/SHUFFLE-SPEC.md, which makes it the
  # only thing in the tree that can catch poker_core and the canister being wrong
  # together. A second opinion nobody runs is one opinion.
  step "[8/9] archive analyser self-tests (no replica, no network)"
  timed_step "$BOUND_ARCHIVE" "archive self-tests" cmd_archive \
    || failed+=("archive analyser self-tests")

  # THE SEALED-DEALER SPIKE'S HARNESS, WHICH NOTHING RAN (docs/DEFECTS.md H-53).
  # Five [[test]] targets, 36 tests, named in their own Cargo.toml so that nothing
  # would be auto-discovered -- and then named by no target at all, in the wave that
  # closed H-45. ~30 s once built. See cmd_no_peeking for why a spike is in the
  # default gate.
  step "[9/9] sealed-dealer spike harness (no replica needed: PocketIC)"
  timed_step "$BOUND_NO_PEEKING" "no-peeking harness" cmd_no_peeking \
    || failed+=("no-peeking harness")

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
  # ONE LIST, NOT TWO (docs/DEFECTS.md H-47). This used to name the self-tests
  # again, in a second copy that had already drifted from tools/shots/package.json:
  # `test-solvency` was in neither this list nor `make test`, and `test-dock-overflow`
  # -- the E-63 gate -- was missing from the package file's. Both callers now invoke
  # tools/shots/selftest.mjs, which DISCOVERS every `test-*.mjs` (so a new gate cannot
  # be unwired) and REQUIRES the named ones to exist (so an old gate cannot vanish).
  node tools/shots/selftest.mjs || { warn "screenshot-harness self-tests FAILED"; return 1; }
  ok "all screenshot-harness self-tests green"
}

# ---------------------------------------------------------------------------
# cmd: cycles  -- how long before a canister stops honouring withdrawals
# ---------------------------------------------------------------------------
#
# docs/DEFECTS.md E-55. A canister below its freezing threshold rejects EVERY
# update call at once, so every player at that table loses access to their own
# money at the same moment with no attacker involved. Nothing in this tree tops a
# canister up, and until wave 13 nothing measured the runway either.
#
# Read-only and anonymous: `get_cycle_status` is a public query, so this needs no
# identity and cannot deploy. `.github/workflows/cycles-monitor.yml` runs the same
# script against mainnet twice a day and `scripts/assert-read-only.sh` enforces
# that neither of them can do anything else.
cmd_cycles() {
  step "cycle runway (local, read-only)"
  gateway_is_up || die "the local replica gateway at $GATEWAY_ORIGIN is not answering. \
Run '$0 local-up' first."
  ./scripts/cycles-runway.sh --selftest || return 1
  echo
  ./scripts/cycles-runway.sh --network local "$@"
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

# THE CUSTODY DISCLOSURE (docs/SECURITY-FINDINGS.md FINDING 23).
#
# Separate from the four above because it is a separate promise and must be able
# to fail on its own. The four say the software is unsafe; these say WHO can take
# your money, which the project did not disclose at all for nine waves while the
# README sold "fully decentralized" and "fair play without requiring trust" and
# disclosed only that a controller can destroy the HAND HISTORY. A controller can
# destroy your BALANCE, an auditor did it for 5 ICP of her own, and neither
# document said so.
#
# Presence checks, like the four: making the disclosure MORE prominent is always
# allowed. The phrases are chosen to be the load-bearing CLAIM rather than any
# particular sentence, so a rewrite that keeps the meaning keeps the gate green
# and a rewrite that drops the meaning cannot.
#
# `## Who can take your money` carries its `## ` on purpose. Without it the check
# was satisfied by the two cross-REFERENCES to the section further down the file,
# so deleting the section itself and leaving the links dangling read green --
# measured, not guessed. A gate that a broken link can satisfy is not a gate.
declare -a README_CUSTODY=(
  "install_code --mode reinstall"
  "uninstall_code"
  "It is **not** trustless"
  "## Who can take your money"
  # WAVE 12. The disclosure has to say THEFT, not only destruction. See below.
  "into a wallet the operator owns"
)
declare -a FRONTEND_CUSTODY=(
  "One key can zero this balance"
  "no restore path"
  "The shuffle needs no trust. Custody does."
  # WAVE 12. Same reason, on the screen the money leaves from.
  "into a wallet the operator owns"
)

# THE TWO SENTENCES THAT MUST NEVER COME BACK (docs/SECURITY-FINDINGS.md FINDING 23c).
#
# Wave 12 shipped a custody disclosure whose last clause was FALSE and false in the
# operator's favour: it told a depositing player, on the deposit screen and in the
# README's decision table, that the operator "cannot pay it to themselves" and that
# the worst case was destruction rather than theft. It is theft. A controller is not
# bound to the ClearDeck wasm: `install_code` installs whatever module it is handed
# and that module can spend the canister's ledger account. Reproduced at
# 39.99990000 ICP by tests/money_safety/tests/controller_custody.rs.
#
# A presence check cannot catch this, because the false sentence was ADDITIONAL
# reassurance sitting next to four true ones -- every phrase the gate looked for was
# present while the paragraph as a whole lied. So the retraction needs its own,
# INVERTED check, the same shape as the "fully decentralized" one below: these exact
# strings, case-sensitive, must not appear in player-facing copy. Case-sensitive on
# purpose, so this file and the finding can still discuss the retraction in prose.
declare -a RETRACTED_CUSTODY_CLAIMS=(
  "cannot pay it to themselves"
  "Destruction, not theft"
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

  # WHO CAN TAKE YOUR MONEY, AS A GATE (docs/SECURITY-FINDINGS.md FINDING 23).
  #
  # This is a check on a DISCLOSURE, not on a fix: FINDING 23 is open, mainnet
  # still has one key on the controller seat, and the only thing standing between
  # a player and that is being told. A disclosure nothing checks is a disclosure
  # that gets tidied away in a copy pass -- which is exactly how the README came
  # to say "fully decentralized" over a canister one command could empty.
  step "custody disclosure present (FINDING 23)"
  local phrase
  for phrase in "${README_CUSTODY[@]}"; do
    if grep -qF "$phrase" README.md; then ok "README.md: \"$phrase\""
    else warn "README.md is MISSING the custody disclosure phrase \"$phrase\""; bad=1; fi
  done
  for phrase in "${FRONTEND_CUSTODY[@]}"; do
    if grep -rqF "$phrase" src/cleardeck_frontend/src; then ok "frontend:  \"$phrase\""
    else warn "the deposit screen is MISSING the custody disclosure \"$phrase\""; bad=1; fi
  done

  # The RETRACTION, as its own gate. A disclosure that is wrong in the operator's
  # favour is worse than none: the presence checks above were all green while the
  # paragraph promised a depositor the operator could not take the money.
  step "the retracted custody claim has not come back (FINDING 23c)"
  local claim hits
  for claim in "${RETRACTED_CUSTODY_CLAIMS[@]}"; do
    hits="$(grep -rnF "$claim" README.md src/cleardeck_frontend/src || true)"
    if [ -n "$hits" ]; then
      warn "player-facing copy claims \"$claim\" again; a controller CAN pay the ledger"
      warn "balance to their own wallet (39.99990000 ICP, controller_custody.rs):"
      printf '%s\n' "$hits" | sed 's/^/      /'
      bad=1
    else
      ok "not claimed: \"$claim\""
    fi
  done
  # The claim the disclosure replaced must not come back. "fully decentralized"
  # over a canister one key can empty is the largest unbacked claim this project
  # has shipped.
  # The DENIAL ("it is not fully decentralized") is the disclosure itself, so the
  # check has to distinguish the claim from the retraction. Anything else and the
  # gate fires on the sentence it exists to protect, which is how a gate gets
  # switched off.
  if grep -inE 'fully decentraliz(ed|sed)' README.md | grep -viE 'not fully decentraliz' >/dev/null; then
    warn "README.md claims \"fully decentralized\" again; one key still holds the controller seat:"
    grep -inE 'fully decentraliz(ed|sed)' README.md | grep -viE 'not fully decentraliz' | sed 's/^/      /'
    bad=1
  else
    ok "README.md does not claim \"fully decentralized\""
  fi

  # THE TYPOGRAPHIC RULE, AS A GATE. No em-dash anywhere in the frontend
  # source: on-screen copy, aria labels, placeholders or comments (the UI
  # wave's rule; the lobby's "Open, no one seated" and the replay's "risk:
  # your funds" used to carry them). The one legitimate code point is the
  # regex character class in lib/utils.js that STRIPS a dash-suffixed name,
  # which is excused by its own pattern, not by file.
  step "no em-dash in the frontend source"
  local dashes
  dashes="$(grep -rn $'\xe2\x80\x94' src/cleardeck_frontend/src \
             --include='*.svelte' --include='*.js' --include='*.scss' \
             | grep -vF '[-' || true)"
  if [ -n "$dashes" ]; then
    warn "em-dash in the frontend source (write a colon, a comma or a period):"
    printf '%s\n' "$dashes" | sed 's/^/      /'
    bad=1
  else
    ok "no em-dash in src/cleardeck_frontend/src"
  fi

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
# cmd: custody   (owner: tests/money_safety/**, src/guardian_canister/**)
# ---------------------------------------------------------------------------
#
# THE CONTROLLER SEAT, on its own. docs/SECURITY-FINDINGS.md FINDING 23.
#
# `dev.sh test` runs this too; this target exists so the FINDING 23 evidence can
# be produced in about twenty seconds instead of inside a twenty-minute gate, and
# so the transcript -- the auditor's wipe with numbers, then every management verb
# the operator can and cannot reach once the guardian holds the seat -- is
# READABLE. Every claim in the README's custody section is a line of this output.

cmd_custody() {
  step "build the table canister wasm (the module the wipe is measured against)"
  cmd_wasm

  step "build the guardian canister"
  ./scripts/build-guardian.sh

  # THE CENSUS READS THE COMMITTED .did, SO THE COMMITTED .did HAS TO BE TRUE.
  #
  # The guardian's entire safety claim is "no destructive verb is on its wire",
  # and the three gates that assert it -- the census, the name list and the
  # Candid-driven sweep -- all parse src/guardian_canister/guardian_canister.did.
  # Nothing compared that file to the module. A wave-12 critic compiled a real
  # `install_chunked_code(mode = Reinstall)` into the guardian, left the .did as
  # committed, and all nine tests passed GREEN over a canister that can wipe a
  # funded table. Checking the interface FIRST is what makes the rest of this
  # target a statement about the module rather than about a text file.
  step "the guardian's committed interface describes the guardian wasm"
  ./scripts/check-candid.sh --guardian-only \
    || die "the guardian's committed .did does not describe its wasm; every custody test below \
would be reading an interface the module does not have"

  step "controller custody: the defect, the premise, and the guardian"
  announce_wasm
  (
    cd tests/money_safety
    export CLEARDECK_TABLE_WASM="$WASM_PATH"
    cargo test --test controller_custody -- --test-threads=2 --nocapture
  ) || die "controller custody gate FAILED"
  ok "controller custody gate passed"
}

# ---------------------------------------------------------------------------
# cmd: archive  -- the offline hand-history analyser's own gates
# ---------------------------------------------------------------------------
#
# docs/DEFECTS.md H-50. `tools/archive/**` is an independent reimplementation of
# docs/SHUFFLE-SPEC.md: it reconstructs any archived hand from its seed, recovers
# the hole cards no record publishes, replays the betting and cross-checks the
# money five ways, and it shares no line with poker_core, the canister or
# src/declarations -- so it can catch them being wrong. It shipped with 39 offline
# self-tests and nothing that ran them.
#
# NO REPLICA AND NO NETWORK. `selftest/run.mjs` builds every population it judges,
# so each answer is known by construction. The fetch step is the only networked
# file in the tool and it is not on this path.
cmd_archive() {
  step "archive analyser self-tests (no replica, no network)"
  require_cmd node
  node tools/archive/selftest/run.mjs || die "archive analyser self-tests FAILED"
  ok "archive analyser self-tests passed"
}

# ---------------------------------------------------------------------------
# cmd: no-peeking  -- the sealed-dealer spike's own harness
# ---------------------------------------------------------------------------
#
# docs/DEFECTS.md H-53. `src/no_peeking/**` and `tests/no_peeking/**` arrived with
# docs/NO-PEEKING-FEASIBILITY.md §10: a sealed dealer canister with an EMPTY
# controller list, a table that holds no cards, and 36 tests that attack them --
# every exported method of both canisters called as the controller, as an opponent,
# as a stranger and anonymously, plus the canister-snapshot read that defeats the
# obvious fix. Five `[[test]]` targets, named in their own Cargo.toml precisely so
# nothing would be auto-discovered, and then **named by no target, no make rule and
# no CI job** -- H-45's shape for the third time, in the wave that closed H-45.
#
# It is a SPIKE. Nothing here is deployed, nothing is in icp.yaml, and the crate is
# detached from the root workspace, so it cannot move a deployed module hash. It is
# in `test` anyway, because the document that cites it makes design claims about
# where ClearDeck's cards could live, and a claim held by an unrun harness is a
# claim held by nothing.
#
# Several of its tests are the FINDING 23 pattern: they PASS while a defect in the
# spike is live (`two_concurrent_try_advance_calls_pay_the_pot_out_twice`,
# `a_zero_in_the_install_argument_reopens_the_whole_hole`). Read the names.
cmd_no_peeking() {
  if [ ! -f tests/no_peeking/Cargo.toml ]; then
    warn "tests/no_peeking is absent; the sealed-dealer spike has no harness to run"
    return 0
  fi
  step "sealed-dealer spike: build the two modules it attacks"
  ./src/no_peeking/build.sh >/dev/null || die "no-peeking spike build FAILED"
  step "sealed-dealer spike: 36 tests, every door, every caller (docs/NO-PEEKING-FEASIBILITY.md)"
  ( cd tests/no_peeking && cargo test --release ) || die "no-peeking harness FAILED"
  # THE SPIKE'S OWN UNIT TESTS, WHICH NOTHING RAN (docs/DEFECTS.md H-23, H-53).
  #
  # H-53 wired the five INTEGRATION targets in tests/no_peeking. The three crates
  # under src/no_peeking are a separate workspace with `#[cfg(test)]` modules of
  # their own -- 11 tests, 0.4 s -- and `cargo test --test X` does not run `--lib`,
  # so no runner in the repo touched them. Found by extending
  # scripts/check-suite-wiring.sh to unit tests after it was written, on the
  # assumption that a new gate is blind somewhere; it was blind here.
  step "sealed-dealer spike: the dealer crates' own unit tests (11, ~0.4 s)"
  ( cd src/no_peeking && cargo test --locked ) || die "no-peeking spike unit tests FAILED"
  ok "no-peeking harness passed"
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
  ${B}local-lobby-sync${R} copy every table contract's config into its lobby record
                    (T-11; local-up runs it, this re-runs it on a live stack)
  ${B}wasm${R}            build table_canister.wasm and print its sha256

  ${B}test${R}            THE PRIMARY GATE, nine steps, every one time-bounded
                  (docs/DEFECTS.md H-42). Step 1 is the cheapest and goes first:
                  every cargo test target on disk must be named by something
                  (H-45) and every Candid binding must regenerate to what is
                  committed (D-11); a tree that fails either does not get to run
                  its gates. Then the workspace tests, the wasm, the differential
                  fast subset, the eighteen money-safety targets (which no longer
                  short-circuit: a red target cannot skip the ones after it),
                  a short fuzz run AND the fuzzer at its own defaults (H-28),
                  the settlement oracle, and the shots/archive/no-peeking
                  self-tests. No replica needed.
  ${B}custody${R}         THE CONTROLLER SEAT, on its own, with the transcript
                  (docs/SECURITY-FINDINGS.md FINDING 23). Reproduces the auditor's
                  wipe -- one 'install_code --mode reinstall' with the same wasm
                  destroying 40 ICP of player claims while the ledger keeps every
                  e8 -- then measures what the guardian canister can and cannot
                  stop. Included in 'test'; separate because the evidence is worth
                  reading and takes about 20 s. Also checks the guardian's
                  committed .did against the guardian wasm FIRST, because every
                  test below it parses that file (docs/DEFECTS.md H-52).
  ${B}archive${R}         the offline hand-history analyser's own 39 gates
                  (tools/archive, docs/ARCHIVE.md). An independent reimplementation
                  of docs/SHUFFLE-SPEC.md that reconstructs hole cards no record
                  publishes. No replica, no network. Included in 'test';
                  docs/DEFECTS.md H-50 is why it has a target.
  ${B}no-peeking${R}      the sealed-dealer spike's own 36 tests
                  (docs/NO-PEEKING-FEASIBILITY.md §10). A SPIKE: nothing here is
                  deployed and nothing is in icp.yaml. Included in 'test';
                  docs/DEFECTS.md H-53 is why it has a target.
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
                  money parser, the NO-RAKE gate, the action dock's containment,
                  the solvency reader and the table-in-frame verdict. ONE list,
                  discovered from disk and cross-checked against a required set,
                  shared with 'npm run selftest' (docs/DEFECTS.md H-47). No replica.
  ${B}cycles${R}          how long before each local canister stops honouring withdrawals
                  (docs/DEFECTS.md E-55). Self-tests the threshold logic on
                  fixtures first, then reads every canister's own get_cycle_status
                  -- a PUBLIC query, so this holds no identity and cannot deploy.
                  Fails below 60 days of MEASURED runway, and treats a canister
                  that will not answer as the alarm rather than as an error:
                  a frozen canister rejects queries too (FINDING 24).
  ${B}known-defects${R}   run the markers that are RED on purpose; succeeds while the
                  engine defects are still present, shouts when one is fixed
  ${B}hygiene${R}         no large/binary files added, no tracked artifact blob, every
                  recorded screenshot red acknowledged; disclaimer, 18+,
                  jurisdiction and no-rake notices present and not weakened
                  since ${BASELINE_COMMIT}
  ${B}selftest${R}        prove the mainnet guard refuses every hostile argument shape

  Standalone gates, all cheap, all also reachable as make targets:
    ${B}make suite-wiring${R}     every cargo test target on disk is run by something
                          (docs/DEFECTS.md H-45). Step [1/9] of 'test' runs it too.
    ${B}make declarations${R}     the frontend's Candid bindings must regenerate to
                          exactly what is committed (docs/DEFECTS.md D-11).
                          '--write' is the ONLY sanctioned way to change them.
    ${B}make deployed-config${R}  the live TableConfig vs icp.yaml AND every lobby
                          row vs its own table contract (L-04), with the extractor
                          self-test that H-55 is why it has (it used to compare
                          one field in eight and say "matches"). LOCAL by default.

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
    local-lobby-sync) cmd_local_lobby_sync ;;
    wasm)           cmd_wasm ;;
    test)           cmd_test ;;
    custody)        cmd_custody ;;
    archive)        cmd_archive ;;
    no-peeking)     cmd_no_peeking ;;
    fuzz)           cmd_fuzz ;;
    fuzz-default)   cmd_fuzz_default ;;
    settlement)     cmd_settlement "$@" ;;
    diff-full)      cmd_diff_full "$@" ;;
    shots)          cmd_shots "$@" ;;
    shots-verdict)  cmd_shots_verdict ;;
    shots-selftest) cmd_shots_selftest ;;
    cycles)         cmd_cycles "$@" ;;
    known-defects)  cmd_known_defects ;;
    hygiene)        cmd_hygiene ;;
    selftest)       cmd_selftest ;;
    phe-venv)       cmd_phe_venv ;;
    *) die "unknown command '$cmd' (try '$0 help')" ;;
  esac
}

main "$@"
