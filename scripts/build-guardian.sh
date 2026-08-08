#!/usr/bin/env bash
#
# Build the GUARDIAN canister (docs/SECURITY-FINDINGS.md FINDING 23) and print its
# identity.
#
#   ./scripts/build-guardian.sh            build, print sha256 and path
#   ./scripts/build-guardian.sh --did      also re-extract guardian_canister.did
#   ./scripts/build-guardian.sh --env      print CLEARDECK_GUARDIAN_WASM=<path>
#
# WHY THIS IS NOT `cargo build -p guardian_canister` FROM THE ROOT
#
# src/guardian_canister carries its own `[workspace]` table, its own Cargo.lock
# and its own target/, so it is NOT a member of the root workspace and the root
# Cargo.lock is untouched by its existence. That is deliberate: mainnet's six
# backend modules currently match a reproducible `cargo build --locked` 6 of 6,
# and the root lockfile is an input to that build. A crate that cannot change the
# root lockfile cannot change those hashes, so "the deployed modules still
# reproduce" needs no argument and no re-verification.
#
# The cost is that `icp build` cannot see this crate through the vendored
# recipes/rust-reproducible.hbs, which runs `cargo build --package <p>` at the
# repo root. Deploying the guardian is therefore a two-step: build here, then
# `icp canister install` the artifact. See README.md, "Who can take your money".

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# $CLEARDECK_GUARDIAN_CRATE lets the money-safety harness build the VARIANT tree
# through this same script. That is not a convenience: it is what makes "the
# module the gate runs" and "the module this script produces" the same bytes by
# construction rather than by two copies of a RUSTFLAGS line agreeing. They did
# not agree, and the gate spent a run testing a module nobody could rebuild
# (docs/DEFECTS.md H-01 is the same shape).
CRATE="${CLEARDECK_GUARDIAN_CRATE:-$REPO_ROOT/src/guardian_canister}"
WASM="$CRATE/target/wasm32-unknown-unknown/release/guardian_canister.wasm"

if [ -t 1 ]; then B=$'\033[1m'; R=$'\033[0m'; G=$'\033[32m'; E=$'\033[31m'
else B=""; R=""; G=""; E=""; fi

die() { printf '\n%sFATAL:%s %s\n' "$E" "$R" "$*" >&2; exit 1; }

command -v cargo >/dev/null 2>&1 || die "cargo not found"

MODE="${1:-}"

if [ "$MODE" != "--env" ]; then
  printf '\n%s==> build guardian_canister -> wasm32-unknown-unknown/release%s\n' "$B" "$R"
fi

# Same reproducibility flags the vendored recipe uses for the fund-holding
# canisters, for the same reason: an absolute path must never reach the module.
CARGO_HOME_ABS="${CARGO_HOME:-$HOME/.cargo}"
export RUSTFLAGS="-Cstrip=symbols --remap-path-prefix=${CARGO_HOME_ABS}=/cargo --remap-path-prefix=${CRATE}=/cleardeck-guardian"

if [ "$MODE" = "--env" ]; then
  ( cd "$CRATE" && cargo build --target wasm32-unknown-unknown --release --locked >/dev/null 2>&1 ) \
    || ( cd "$CRATE" && cargo build --target wasm32-unknown-unknown --release >/dev/null )
else
  ( cd "$CRATE" && cargo build --target wasm32-unknown-unknown --release --locked ) \
    || ( cd "$CRATE" && cargo build --target wasm32-unknown-unknown --release )
fi

[ -f "$WASM" ] || die "cargo reported success but $WASM is absent"

if [ "$MODE" = "--did" ]; then
  command -v candid-extractor >/dev/null 2>&1 \
    || die "candid-extractor not found -- cargo install candid-extractor"
  ( cd "$CRATE" && candid-extractor "$WASM" > guardian_canister.did )
  printf '    %s✓%s regenerated %s\n' "$G" "$R" "$CRATE/guardian_canister.did"
fi

if [ "$MODE" = "--env" ]; then
  printf 'CLEARDECK_GUARDIAN_WASM=%s' "$WASM"
  exit 0
fi

printf '    %s✓%s %s  %s\n' "$G" "$R" "$(shasum -a 256 "$WASM" | awk '{print $1}')" "$WASM"
printf '    %s bytes\n' "$(wc -c < "$WASM" | tr -d ' ')"
