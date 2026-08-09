#!/usr/bin/env bash
# Do the frontend's Candid BINDINGS describe the canisters they build actors from?
#
# WHY THIS EXISTS
#   docs/DEFECTS.md D-11. `src/cleardeck_frontend/src/lib/canisters.js` builds every
#   actor from `src/declarations/<n>/<n>.did.js`. That file is a THIRD copy of each
#   interface -- after the Rust source and the committed `.did` -- and it was
#   hand-maintained, so it drifted from both. Measured 2026-08-09:
#
#     table_1  canister .did 75 methods, .did.js 61
#              MISSING 16: claim_external_deposit, get_solvency, refresh_solvency,
#              get_all_ledger_intents, get_deposit_replay_state, get_deposit_custody,
#              refund_external_deposit, resolve_ledger_intent, ...
#              EXTRA 2: admin_restore_balance, deposit_from_external -- methods the
#              canister does NOT have, so a client calling them fails at the wire
#     history  18 vs 12, MISSING 6
#     lobby    26 vs 24 (currency declared `opt Currency` against a bare Currency)
#
#   Nothing failed loudly. The frontend guards its calls
#   (`if (!tableActor?.get_custody_status)`), so a MISSING BINDING reads as a
#   missing feature rather than as a broken build: the custody and solvency
#   instruments built in waves 7-10 were simply unreachable from the UI, and the
#   register recorded them as shipped.
#
#   `scripts/check-candid.sh` already answers this question for the other two
#   copies (committed `.did` vs the built wasm, and `src/declarations/<n>/<n>.did`
#   vs the canister `.did`). This is the missing third leg.
#
# WHAT IT DOES
#   Regenerates each `.did.js` and `.did.d.ts` from the committed canister `.did`
#   with `tools/gen-declarations` -- the same `candid_parser` bindings `didc bind`
#   uses -- and DIFFS. Any difference fails. `--write` accepts the regeneration.
#
#   It is a diff against a generator rather than a hand-written comparison on
#   purpose: a comparison can be wrong about what it compares (see
#   check-deployed-config.sh, whose extractor silently skipped seven fields out of
#   eight), whereas "regenerate and diff" has nothing to be wrong about.
#
# USAGE
#   ./scripts/check-declarations-js.sh            # fail on any drift
#   ./scripts/check-declarations-js.sh --write    # regenerate and accept
#   ./scripts/check-declarations-js.sh --selftest # prove the gate can go red
#
#   It reads and writes only files in this repository. No network, no canister call.
set -euo pipefail
cd "$(dirname "$0")/.."

WRITE=0
SELFTEST=0
while [ $# -gt 0 ]; do
  case "$1" in
    --write)    WRITE=1; shift ;;
    --selftest) SELFTEST=1; shift ;;
    -h|--help)  sed -n '2,44p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

GEN=tools/gen-declarations/target/release/gen-declarations
if [ ! -x "$GEN" ]; then
  echo "building tools/gen-declarations ..."
  ( cd tools/gen-declarations && cargo build --release --offline >/dev/null 2>&1 ) \
    || ( cd tools/gen-declarations && cargo build --release >/dev/null )
fi
[ -x "$GEN" ] || { echo "could not build $GEN" >&2; exit 1; }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# <declarations dir name>:<crate holding the .did>
#
# frontend and internet_identity are deliberately absent: neither has a `.did` in
# this repository (one is the asset canister, one is a third-party canister), so
# there is nothing here to generate them FROM and a gate that pretended otherwise
# would be checking a file against itself.
PAIRS="lobby:lobby_canister history:history_canister table_1:table_canister table_2:table_canister table_3:table_canister"

fail=0
checked=0
for pair in $PAIRS; do
  n="${pair%%:*}"; c="${pair##*:}"
  src="src/$c/$c.did"
  dir="src/declarations/$n"
  [ -f "$src" ] || { echo "  $n: $src is missing, skipping"; continue; }
  [ -d "$dir" ] || { echo "  $n: $dir is missing, skipping"; continue; }

  "$GEN" "$src" "$TMP/$n.did.js" "$TMP/$n.did.d.ts" >/dev/null
  checked=$((checked + 1))

  drift=0
  for ext in did.js did.d.ts; do
    target="$dir/$n.$ext"
    if [ ! -f "$target" ]; then
      echo "  ✗ $target does not exist"
      drift=1; continue
    fi
    if ! diff -q "$TMP/$n.$ext" "$target" >/dev/null; then
      drift=1
      if [ "$WRITE" = "1" ]; then
        cp "$TMP/$n.$ext" "$target"
        echo "  ~ rewrote $target"
      else
        echo "  ✗ $target does not match $src"
        # Captured, not piped straight into `sed -n '1,40p'`: under `pipefail`
        # that closes the pipe early, `diff` dies of SIGPIPE and the whole script
        # exits after the FIRST drifted file -- so the report would name one
        # binding and stop, which is how a gate under-reports and gets believed.
        d="$(diff -u "$target" "$TMP/$n.$ext" || true)"
        printf '%s\n' "$d" | head -40 | sed 's/^/      /'
        [ "$(printf '%s\n' "$d" | wc -l)" -le 40 ] || echo "      ... (truncated)"
      fi
    fi
  done
  if [ "$drift" -eq 0 ]; then
    echo "  ✓ $dir matches $src"
  elif [ "$WRITE" != "1" ]; then
    fail=1
  fi
done

# THE GATE MUST BE ABLE TO GO RED. An instrument that measures nothing passes, and
# this repository has that failure nine times over, so the self-test plants a
# realistic drift -- a method deleted from a binding, which is D-11's exact
# shape -- and requires a conviction.
if [ "$SELFTEST" = "1" ]; then
  echo
  echo "self-test: plant D-11's own shape (a method missing from a binding)"
  victim="src/declarations/lobby/lobby.did.js"
  backup="$TMP/lobby.did.js.orig"
  cp "$victim" "$backup"
  # Remove one method line from the service block.
  python3 - "$victim" <<'PY'
import re, sys
p = sys.argv[1]
s = open(p).read()
s2 = re.sub(r"\n\s*'get_tables'\s*:\s*IDL\.Func\([^\n]*\n", "\n", s, count=1)
assert s2 != s, "self-test could not plant the drift: 'get_tables' not found"
open(p, "w").write(s2)
PY
  if "$0" >/dev/null 2>&1; then
    cp "$backup" "$victim"
    echo "  FAIL  a binding with get_tables deleted was reported as matching" >&2
    exit 1
  fi
  cp "$backup" "$victim"
  echo "  ok    a binding with get_tables deleted is convicted"
  if ! "$0" >/dev/null 2>&1; then
    echo "  FAIL  the restored tree is still red, so the self-test did not restore it" >&2
    exit 1
  fi
  echo "  ok    the restored tree is green again"
  echo "self-test passed"
  echo
fi

echo
if [ "$fail" -ne 0 ]; then
  cat <<'EOF'
THE FRONTEND'S CANDID BINDINGS DO NOT DESCRIBE THE CANISTERS.

src/declarations/<n>/<n>.did.js is what every actor in the app is built from. A
method missing here is a method the UI CANNOT CALL, and the app hides that: it
guards its calls, so an absent binding looks like an absent feature. A method
present here that the canister does not export fails at the wire instead.

Fix by regenerating, never by hand:

    ./scripts/check-declarations-js.sh --write

If the generated file looks wrong, the .did is wrong -- fix that first and let
./scripts/check-candid.sh judge it against the built wasm.
EOF
  exit 1
fi
echo "declarations: all $checked binding set(s) regenerate to exactly what is committed"
