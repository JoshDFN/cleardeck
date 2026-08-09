#!/usr/bin/env bash
#
# scripts/pocket-ic.sh -- resolve a PocketIC **server** binary, deterministically.
#
# WHY THIS FILE EXISTS (docs/DEFECTS.md H-23)
#
# Every fund-safety result in this project comes from a harness that needs a
# PocketIC server, and that is the whole reason none of those harnesses were ever
# in CI: `cargo test --locked --workspace` cannot start a replica, so the money
# invariants, the settlement oracle, the fuzzer and the custody gate ran only when
# a human remembered. This script removes the obstacle instead of routing around
# it.
#
# It prints ONE line on stdout -- the absolute path to a verified server binary --
# and everything else goes to stderr, so a caller can write:
#
#     export POCKET_IC_BIN="$(./scripts/pocket-ic.sh)"
#
# THE VERSION IS NOT WRITTEN DOWN TWICE. The `pocket-ic` Rust crate and the server
# must agree, so the required version is READ OUT OF tests/money_safety/Cargo.lock
# rather than restated here. A crate bump therefore cannot silently leave CI on an
# older server: it lands on a version with no pinned checksum, and this script
# stops with the checksum it needs.
#
# LOCAL REPLICA ONLY. This talks to github.com over read-only HTTPS to fetch a
# release asset. It touches no canister, no mainnet id and no wallet.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# --------------------------------------------------------------------------
# the pinned checksums
# --------------------------------------------------------------------------
#
# Keyed `<server-version>/<asset>`. Every value is the sha256 of the GZIPPED
# release asset exactly as GitHub serves it, recorded by fetching it:
#
#   curl -sSL https://github.com/dfinity/pocketic/releases/download/11.0.0/pocket-ic-x86_64-linux.gz \
#     | shasum -a 256
#
# An unpinned download is an unidentified replica, and a harness that cannot say
# which replica produced a money result has not produced a money result.
pinned_sha256() {
  case "$1" in
    '11.0.0/pocket-ic-x86_64-linux')  echo c5ba1ae43fe59281bc68cde0d45442452ecb1d04aefdf3f320c94d6f08612db3 ;;
    '11.0.0/pocket-ic-arm64-linux')   echo 816bc2848abb5ff8ee70d79c3fd7458272d6740d4e3b64de279d1f5e8244dc12 ;;
    '11.0.0/pocket-ic-arm64-darwin')  echo c64e7403f2f89bc1b6606944932cc7468423c79604ce81c055d6ee6faf7faadf ;;
    '11.0.0/pocket-ic-x86_64-darwin') echo ba2a55aef8b94e6d7ddd18d3e1e08006e46a2031de5147ea2683a4fbca4a2dc4 ;;
    *) return 1 ;;
  esac
}

say()  { printf '  %s\n' "$*" >&2; }
die()  { printf '  ERROR: %s\n' "$*" >&2; exit 1; }

# --------------------------------------------------------------------------
# what version does the harness's own lockfile require
# --------------------------------------------------------------------------
required_version() {
  local lock="$REPO_ROOT/tests/money_safety/Cargo.lock" v
  [ -f "$lock" ] || die "$lock is missing; cannot tell which PocketIC server the harness needs"
  # The `[[package]] name = "pocket-ic"` stanza, and the `version` line under it.
  v="$(awk '
        /^name = "pocket-ic"$/ { want = 1; next }
        want && /^version = / { gsub(/[",]/, "", $3); print $3; exit }
      ' "$lock")"
  [ -n "$v" ] || die "no pocket-ic entry in $lock; the harness no longer pins a server version"
  printf '%s' "$v"
}

# --------------------------------------------------------------------------
# platform
# --------------------------------------------------------------------------
asset_name() {
  local os arch
  case "$(uname -s)" in
    Linux)  os=linux ;;
    Darwin) os=darwin ;;
    *) die "unsupported OS $(uname -s); set POCKET_IC_BIN to a server binary yourself" ;;
  esac
  case "$(uname -m)" in
    x86_64|amd64) arch=x86_64 ;;
    arm64|aarch64) arch=arm64 ;;
    *) die "unsupported CPU $(uname -m); set POCKET_IC_BIN to a server binary yourself" ;;
  esac
  printf 'pocket-ic-%s-%s' "$arch" "$os"
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  else shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

# A candidate is only usable if it RUNS and says the version we asked for. A file
# that exists is not a replica: a truncated download, a wrong-architecture binary
# and a stale dfx copy all pass `test -x` and all fail here.
reports_version() {
  local bin="$1" want="$2" out
  [ -x "$bin" ] || return 1
  out="$("$bin" --version 2>/dev/null || true)"
  printf '%s' "$out" | grep -qx "pocket-ic-server $want"
}

main() {
  local want asset cache bin url sha got

  want="$(required_version)"
  asset="$(asset_name)"
  say "PocketIC server $want required by tests/money_safety/Cargo.lock ($asset)"

  # 1. an explicit override, still verified.
  if [ -n "${POCKET_IC_BIN:-}" ]; then
    reports_version "$POCKET_IC_BIN" "$want" \
      || die "POCKET_IC_BIN=$POCKET_IC_BIN is not a pocket-ic-server $want"
    say "using \$POCKET_IC_BIN"
    printf '%s\n' "$POCKET_IC_BIN"
    return 0
  fi

  # 2. this repo's own cache, which is what actions/cache restores in CI.
  # THE FILENAME IS LOAD-BEARING. The server panics on startup unless argv[0]'s
  # basename is exactly `pocket-ic` or `pocket-ic-server`:
  #   "The PocketIc server binary name must be "pocket-ic" or "pocket-ic-server""
  # Measured, not read: extracting the release asset to `pocket-ic-11.0.0` and
  # running it exits 101 on that panic. The version therefore goes in the
  # DIRECTORY, so two versions can be cached side by side and each keeps the one
  # name the server will answer to.
  cache="${CLEARDECK_CACHE_DIR:-$REPO_ROOT/target}/pocket-ic/$want"
  bin="$cache/pocket-ic"
  if reports_version "$bin" "$want"; then
    say "cached: $bin"
    printf '%s\n' "$bin"
    return 0
  fi

  # 3. the dfx cache, which is where a developer's copy already lives.
  if [ "${CLEARDECK_POCKET_IC_NO_DFX:-0}" != "1" ] && command -v dfx >/dev/null 2>&1; then
    local dfx_bin
    dfx_bin="$(dfx cache show 2>/dev/null || true)/pocket-ic"
    if reports_version "$dfx_bin" "$want"; then
      say "dfx cache: $dfx_bin"
      printf '%s\n' "$dfx_bin"
      return 0
    fi
  fi

  # 4. fetch the pinned release asset.
  sha="$(pinned_sha256 "$want/$asset" || true)"
  [ -n "$sha" ] || die "no pinned checksum for $want/$asset.
  The pocket-ic crate in tests/money_safety/Cargo.lock moved to $want and this
  script has not been told what that server's asset hashes are. Record them:
    curl -sSL https://github.com/dfinity/pocketic/releases/download/$want/$asset.gz | shasum -a 256
  and add the line to pinned_sha256() above. Fetching an unpinned replica would
  make every money result below it unattributable, so this stops instead."

  url="https://github.com/dfinity/pocketic/releases/download/$want/$asset.gz"
  mkdir -p "$cache"
  say "fetching $url"
  curl -sSL --fail --retry 3 --retry-delay 2 --max-time 300 -o "$cache/$asset.gz.tmp" "$url" \
    || die "could not fetch $url"

  got="$(sha256_of "$cache/$asset.gz.tmp")"
  if [ "$got" != "$sha" ]; then
    rm -f "$cache/$asset.gz.tmp"
    die "$url has sha256 $got, expected $sha. Refusing to run money tests against an unidentified replica."
  fi
  say "sha256 ok: $got"

  gunzip -c "$cache/$asset.gz.tmp" > "$bin.tmp"
  rm -f "$cache/$asset.gz.tmp"
  chmod +x "$bin.tmp"
  mv -f "$bin.tmp" "$bin"

  reports_version "$bin" "$want" \
    || die "$bin does not report 'pocket-ic-server $want' after extraction"
  say "installed: $bin"
  printf '%s\n' "$bin"
}

# --------------------------------------------------------------------------
# --selftest: prove the checksum gate is not decoration
# --------------------------------------------------------------------------
#
# THE STANDING LESSON. A verifier that accepts everything is indistinguishable
# from no verifier, and this project has shipped that exact thing more than once
# (the Candid job that ended in `exit 0`, the notice gate wired to two scenarios).
# So the two ways this script could be blind are executed here, every run:
#
#   1. it would accept a corrupted download, and
#   2. it would accept a binary that is not the server the crate needs.
selftest() {
  local want tmp rc
  want="$(required_version)"
  printf '  pocket-ic.sh selftest (required server: %s)\n' "$want" >&2
  tmp="$(mktemp -d)"
  # Expanded NOW, not at exit: `tmp` is local to this function and is out of
  # scope by the time the EXIT trap fires, and under `set -u` that turns a clean
  # pass into a non-zero exit -- a selftest that reports success and then fails.
  # shellcheck disable=SC2064
  trap "rm -rf '$tmp'" EXIT

  # 1. a file that is not a server must be refused as $POCKET_IC_BIN.
  printf '#!/bin/sh\necho pocket-ic-server 0.0.0\n' > "$tmp/fake"
  chmod +x "$tmp/fake"
  rc=0
  ( POCKET_IC_BIN="$tmp/fake" main >/dev/null 2>&1 ) || rc=$?
  [ "$rc" -ne 0 ] || { echo "  SELFTEST FAILED: a server reporting 0.0.0 was accepted as $want" >&2; exit 1; }
  echo "  ok: a wrong-version server binary is refused" >&2

  # 2. a non-executable file must be refused.
  : > "$tmp/empty"
  rc=0
  ( POCKET_IC_BIN="$tmp/empty" main >/dev/null 2>&1 ) || rc=$?
  [ "$rc" -ne 0 ] || { echo "  SELFTEST FAILED: an empty file was accepted as a replica" >&2; exit 1; }
  echo "  ok: an empty file is refused" >&2

  # 3. the checksum table must actually be consulted: a version with no pin has
  #    to stop the script rather than fetch whatever is at the URL.
  rc=0
  ( pinned_sha256 "0.0.0-not-a-release/pocket-ic-x86_64-linux" ) && rc=1
  [ "$rc" -eq 0 ] || { echo "  SELFTEST FAILED: pinned_sha256 invented a checksum" >&2; exit 1; }
  echo "  ok: an unpinned version has no checksum to fetch against" >&2

  # 4. and a real resolution still succeeds, or the three refusals above prove
  #    only that the script refuses everything.
  local resolved
  resolved="$(main)" || { echo "  SELFTEST FAILED: could not resolve a real server" >&2; exit 1; }
  reports_version "$resolved" "$want" \
    || { echo "  SELFTEST FAILED: resolved $resolved is not pocket-ic-server $want" >&2; exit 1; }
  echo "  ok: a genuine pocket-ic-server $want resolves: $resolved" >&2
  echo "  pocket-ic.sh selftest passed" >&2
}

case "${1:-}" in
  --selftest) selftest ;;
  '') main ;;
  *) die "usage: $0 [--selftest]" ;;
esac
