#!/usr/bin/env bash
# Does the committed Candid describe the code that actually ships?
#
# WHY THIS EXISTS
#   `src/<crate>/<crate>.did` is not documentation. `icp build` embeds it verbatim
#   into the module as the PUBLIC `candid:service` metadata (see
#   recipes/rust-reproducible.hbs), so it is the interface every running canister
#   publishes about itself, and it is what third-party tooling reads to build a
#   client. A stale one is a canister lying about its own shape, on-chain.
#
#   It was stale. On 2026-08-06 the committed table_canister.did omitted seven
#   fields and one whole method that the code implements -- among them
#   `TableState.departed_stakes`, the record that stops a departed player's stake
#   from being silently reassigned to the deepest stack. The lobby declared
#   `currency : opt Currency` where the canister exports a bare `Currency`.
#
#   None of that failed loudly. Candid's constituent subtyping decodes a bare
#   value into an `opt` without complaining, and a record with extra fields on
#   the wire decodes fine with the extra fields dropped. So a client built from
#   the committed file read `currency` as null and never saw `sitting_out_since`
#   -- correct-looking replies, missing information, no error anywhere.
#
#   A "Candid interface drift" CI job existed the whole time and passed, because
#   it ended in `exit 0` behind a TODO. It could not be turned on, because it
#   compared BYTES: table_canister.did carries hundreds of lines of hand-written
#   documentation the extractor does not emit (and which ship on-chain in the
#   metadata), and the extractor renumbers `Result_N` aliases at will. The guard
#   was too loud to enable, so it was disabled, so it was silent. This script
#   replaces it with the comparison that was needed all along: STRUCTURAL, over
#   fully-resolved method signatures. See scripts/candid_compare.py.
#
# USAGE
#   ./scripts/check-candid.sh                 # committed .did vs the built wasm
#   ./scripts/check-candid.sh --no-build      # reuse an existing release build
#   ./scripts/check-candid.sh --write         # rewrite the .did from the wasm
#                                             # (LOSES the hand-written comments;
#                                             #  prefer editing the .did by hand)
#   ./scripts/check-candid.sh --declarations  # also check src/declarations
#   ./scripts/check-candid.sh --selftest      # prove the gate can go red
#
#   It only ever reads the local build. It never touches a network.
set -euo pipefail

# candid-extractor's output format is an INPUT to this check. Leaving it
# unpinned (`cargo install candid-extractor --locked`, as CI did) means a new
# upstream release can change the extraction and move the gate under us.
CANDID_EXTRACTOR_VERSION='0.1.6'

BUILD=1
WRITE=0
DECLARATIONS=0
SELFTEST=0
while [ $# -gt 0 ]; do
  case "$1" in
    --no-build)     BUILD=0; shift ;;
    --write)        WRITE=1; shift ;;
    --declarations) DECLARATIONS=1; shift ;;
    --selftest)     SELFTEST=1; shift ;;
    -h|--help)      sed -n '2,40p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

cd "$(dirname "$0")/.."
CMP=scripts/candid_compare.py
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

CRATES="lobby_canister table_canister history_canister"

# --- the gate must be able to go red -----------------------------------------
# A comparison that silently reports "no differences" because it parsed nothing
# is this project's recurring failure mode, so prove the instrument works before
# trusting its verdict.
selftest() {
  echo "== selftest: can the comparator see a planted difference?"
  cat > "$TMP/a.did" <<'EOF'
type R = variant { Ok : nat64; Err : text };
service : () -> { m : (nat8) -> (R) query; }
EOF
  # Same interface, different alias name, different field order, blob sugar,
  # and a comment. MUST be reported as identical.
  cat > "$TMP/b.did" <<'EOF'
// a comment the other file does not have
type Renamed = variant { Err : text; Ok : nat64 };
service : () -> { m : (nat8) -> (Renamed) query; }
EOF
  if ! python3 "$CMP" "$TMP/a.did" "$TMP/b.did" >/dev/null; then
    echo "::error::comparator reports drift between two IDENTICAL interfaces (false positive)"
    exit 1
  fi
  echo "   ok: renaming an alias / reordering a variant / adding a comment is NOT drift"

  # Each of these is real drift and MUST be caught.
  plant() {
    printf '%s\n' "$2" > "$TMP/c.did"
    if python3 "$CMP" "$TMP/a.did" "$TMP/c.did" >/dev/null 2>&1; then
      echo "::error::comparator did NOT catch planted drift: $1"
      exit 1
    fi
    echo "   ok: caught -- $1"
  }
  plant "a dropped field" \
    'type R = variant { Ok : nat64 }; service : () -> { m : (nat8) -> (R) query; }'
  plant "a widened integer" \
    'type R = variant { Ok : nat32; Err : text }; service : () -> { m : (nat8) -> (R) query; }'
  plant "a method that disappeared" \
    'type R = variant { Ok : nat64; Err : text }; service : () -> { }'
  plant "an extra method" \
    'type R = variant { Ok : nat64; Err : text };
     service : () -> { m : (nat8) -> (R) query; extra : () -> (); }'
  plant "an update call demoted to a query" \
    'type R = variant { Ok : nat64; Err : text }; service : () -> { m : (nat8) -> (R); }'
  plant "a changed argument type" \
    'type R = variant { Ok : nat64; Err : text }; service : () -> { m : (text) -> (R) query; }'
  echo "   selftest passed"
  echo
}

[ "$SELFTEST" -eq 1 ] && { selftest; exit 0; }
selftest

# --- extract the truth out of the built wasm ---------------------------------
if ! command -v candid-extractor >/dev/null 2>&1; then
  echo "candid-extractor not found -- run:" >&2
  echo "  cargo install candid-extractor --version $CANDID_EXTRACTOR_VERSION --locked" >&2
  exit 1
fi
have="$(candid-extractor --version 2>/dev/null | awk '{print $NF}')"
if [ "$have" != "$CANDID_EXTRACTOR_VERSION" ]; then
  echo "::warning::candid-extractor $have, this check is pinned to $CANDID_EXTRACTOR_VERSION"
fi

if [ "$BUILD" -eq 1 ]; then
  echo "== building release wasm"
  cargo build --locked --release --target wasm32-unknown-unknown >/dev/null
fi

fail=0
checked=0
echo "== committed .did vs the interface exported by the built wasm"
for crate in $CRATES; do
  wasm="target/wasm32-unknown-unknown/release/$crate.wasm"
  did="src/$crate/$crate.did"
  [ -f "$wasm" ] || { echo "::error::$wasm not built"; exit 1; }
  [ -f "$did" ]  || { echo "::error::$did missing"; exit 1; }

  candid-extractor "$wasm" > "$TMP/$crate.did"
  # An extractor that produced nothing would make every comparison below agree.
  lines=$(wc -l < "$TMP/$crate.did")
  [ "$lines" -gt 20 ] || { echo "::error::extracted only $lines lines from $wasm"; exit 1; }

  if [ "$WRITE" -eq 1 ]; then
    cp "$TMP/$crate.did" "$did"
    echo "  wrote $did from $wasm"
    continue
  fi

  checked=$((checked + 1))
  if python3 "$CMP" "$did" "$TMP/$crate.did" --label-a COMMITTED --label-b WASM; then
    echo "  ok  $did"
  else
    echo "  ✗   $did does not describe $wasm"
    fail=1
  fi
done

if [ "$WRITE" -eq 1 ]; then
  echo "rewrote $(echo $CRATES | wc -w | tr -d ' ') .did files -- re-read the diff before committing"
  exit 0
fi
[ "$checked" -eq 3 ] || { echo "::error::checked $checked interfaces, expected 3"; exit 1; }

# --- the frontend's second copy of the same interface ------------------------
# src/declarations/<n>/<n>.did is generated from the canister .did and committed.
# It is ALSO drifted, and it is not this script's job to fix files it does not
# own, so the known drift is pinned in a baseline: new drift fails, and a
# baseline line that stops being true also fails, so the list can only shrink.
if [ "$DECLARATIONS" -eq 1 ]; then
  echo
  echo "== src/declarations vs the canister .did (baseline-guarded)"
  BASELINE=scripts/candid-declarations-baseline.txt
  : > "$TMP/current.txt"
  for pair in lobby:lobby_canister history:history_canister \
              table_1:table_canister table_2:table_canister table_3:table_canister; do
    n="${pair%%:*}"; c="${pair##*:}"
    [ -f "src/declarations/$n/$n.did" ] || continue
    python3 "$CMP" "src/declarations/$n/$n.did" "src/$c/$c.did" --list \
      | sed "s|^|$n\t|" >> "$TMP/current.txt" || true
  done
  sort -o "$TMP/current.txt" "$TMP/current.txt"

  if [ ! -f "$BASELINE" ]; then
    echo "::error::$BASELINE is missing"
    exit 1
  fi
  grep -vE '^\s*(#|$)' "$BASELINE" | sort > "$TMP/baseline.txt"

  new="$(comm -23 "$TMP/current.txt" "$TMP/baseline.txt" || true)"
  gone="$(comm -13 "$TMP/current.txt" "$TMP/baseline.txt" || true)"
  if [ -n "$new" ]; then
    echo "::error::NEW declaration drift, not in the baseline:"
    printf '%s\n' "$new" | sed 's/^/      /'
    fail=1
  fi
  if [ -n "$gone" ]; then
    echo "::error::these baseline entries no longer drift -- delete them from $BASELINE:"
    printf '%s\n' "$gone" | sed 's/^/      /'
    fail=1
  fi
  [ -n "$new$gone" ] || echo "  ok  declaration drift matches the baseline exactly ($(wc -l < "$TMP/baseline.txt" | tr -d ' ') known items)"
fi

echo
if [ "$fail" -ne 0 ]; then
  cat <<'EOF'
THE COMMITTED CANDID DOES NOT DESCRIBE THE DEPLOYED CODE.

This is not cosmetic. `icp build` embeds the .did into the module as the public
`candid:service` metadata, so whatever is wrong here is what every canister
publishes about itself and what every generated client believes.

To fix, EDIT THE .did BY HAND to match the structure reported above. Do not run
--write unless you mean it: the .did carries hand-written documentation that the
extractor does not reproduce, and that documentation ships on-chain.
EOF
  exit 1
fi
echo "candid: committed interfaces match the built wasm"
