#!/usr/bin/env bash
#
# Build the NO-PEEKING SPIKE (the sealed dealer and the table that holds no cards)
# and print the identity of each module.
#
#   ./src/no_peeking/build.sh            build, print sha256 and path
#   ./src/no_peeking/build.sh --did      also re-extract the two .did files
#
# WHY THIS IS NOT `cargo build -p dealer_canister` FROM THE REPO ROOT
#
# src/no_peeking carries its own `[workspace]` table, its own Cargo.lock and its
# own target/, so it is NOT a member of the root workspace and the root Cargo.lock
# is untouched by its existence. That is the same rule src/guardian_canister and
# tests/money_safety follow, for the same reason: mainnet's six backend modules
# currently match a reproducible `cargo build --locked` 6 of 6, and the root
# lockfile is an input to that build. A crate that cannot change the root lockfile
# cannot change those hashes.
#
# NOTHING HERE IS DEPLOYED. It is deliberately absent from icp.yaml, exactly as
# src/guardian_canister is. See README.md in this directory.

set -euo pipefail

CRATE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$CRATE/target/wasm32-unknown-unknown/release"

if [ -t 1 ]; then B=$'\033[1m'; R=$'\033[0m'; G=$'\033[32m'; E=$'\033[31m'
else B=""; R=""; G=""; E=""; fi

die() { printf '\n%sFATAL:%s %s\n' "$E" "$R" "$*" >&2; exit 1; }

command -v cargo >/dev/null 2>&1 || die "cargo not found"

printf '\n%s==> build no-peeking spike -> wasm32-unknown-unknown/release%s\n' "$B" "$R"

# Same reproducibility flags the vendored recipe uses for the fund-holding
# canisters, for the same reason: an absolute path must never reach the module.
CARGO_HOME_ABS="${CARGO_HOME:-$HOME/.cargo}"
export RUSTFLAGS="-Cstrip=symbols --remap-path-prefix=${CARGO_HOME_ABS}=/cargo --remap-path-prefix=${CRATE}=/cleardeck-no-peeking"

( cd "$CRATE" && cargo build --target wasm32-unknown-unknown --release --locked ) \
  || ( cd "$CRATE" && cargo build --target wasm32-unknown-unknown --release )

for name in dealer_canister table_stub; do
  wasm="$OUT/$name.wasm"
  [ -f "$wasm" ] || die "cargo reported success but $wasm is absent"
  if [ "${1:-}" = "--did" ]; then
    command -v candid-extractor >/dev/null 2>&1 \
      || die "candid-extractor not found -- cargo install candid-extractor"
    candid-extractor "$wasm" > "$CRATE/$name/$name.did"
    printf '    %s✓%s regenerated %s\n' "$G" "$R" "$CRATE/$name/$name.did"
  fi
  printf '    %s✓%s %s  %s\n' "$G" "$R" "$(shasum -a 256 "$wasm" | awk '{print $1}')" "$wasm"
  printf '      %s bytes\n' "$(wc -c < "$wasm" | tr -d ' ')"
done

cat <<'NOTE'

    The dealer is only what this claims to be once its controller list is EMPTY.
    Installing it and leaving a controller on it buys nothing at all. The handover
    is one-way and unrepeatable, so rehearse it the way docs/SECURITY-FINDINGS.md
    FINDING 23 §6 describes, and set the freezing threshold generously FIRST --
    `update_settings` on a zero-controller canister is refused forever.
NOTE
