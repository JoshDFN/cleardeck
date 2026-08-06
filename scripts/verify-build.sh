#!/usr/bin/env bash
# ClearDeck build verification.
# GitHub: https://github.com/JoshDFN/cleardeck
#
# Answers one question: is the code running in these canisters the code in this
# repository? It builds the canisters in a digest-pinned Docker image and
# compares the resulting sha256 of each module against the module hash the
# Internet Computer reports for the live canister.
#
#   ./scripts/verify-build.sh --mainnet          verify the live mainnet canisters
#   ./scripts/verify-build.sh --local            verify your local replica
#   ./scripts/verify-build.sh --two-paths        just prove the build is reproducible
#   ./scripts/verify-build.sh --emit <dir>       build and write the wasms to <dir>
#
# Reading the deployed module hash uses `icp canister status`, which issues a
# management-canister call. On mainnet that call is controller-only, so if you
# are not a controller the script will tell you it could not read the hash and
# point you at the public dashboard, which shows the module hash to anyone.
# Everything else -- the build, the two-path reproducibility check, and reading
# the git:revision each canister claims -- works for a total stranger.
#
# The previous version of this script could not work at all: it drove a
# Dockerfile that did not compile. An independent auditor found that.

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

CANISTERS=(lobby history table_1 table_2 table_3 btc_table_1)
IMAGE_TAG="cleardeck-verify"

MODE=""
REVISION=""
DIRTY=""
EMIT_DIR=""
IDENTITY="${IDENTITY:-}"

die() { printf '\033[31merror\033[0m %s\n' "$*" >&2; exit 1; }
info() { printf '\033[36m==>\033[0m %s\n' "$*"; }

usage() {
    sed -n '2,23p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

while [ $# -gt 0 ]; do
    case "$1" in
        --mainnet)    MODE=mainnet ;;
        --local)      MODE=local ;;
        --two-paths)  MODE=two-paths ;;
        --emit)       MODE=emit; EMIT_DIR="${2:-}"; [ -n "$EMIT_DIR" ] || die "--emit needs a directory"; shift ;;
        --revision)   REVISION="${2:-}"; shift ;;
        --dirty)      DIRTY="${2:-}"; shift ;;
        --identity)   IDENTITY="${2:-}"; shift ;;
        -h|--help)    usage 0 ;;
        *)            die "unknown argument '$1' (try --help)" ;;
    esac
    shift
done

[ -n "$MODE" ] || { echo "Pick a target: --mainnet, --local, --two-paths or --emit <dir>." >&2; echo >&2; usage 2; }

command -v docker >/dev/null 2>&1 || die "docker is required. Install Docker and start the daemon."
docker info >/dev/null 2>&1 || die "the Docker daemon is not running."

# ---------------------------------------------------------------------------
# What revision are we building?
# ---------------------------------------------------------------------------
if [ -z "$REVISION" ]; then
    REVISION="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
fi
if [ -z "$DIRTY" ]; then
    if git rev-parse --git-dir >/dev/null 2>&1; then
        if [ -n "$(git status --porcelain 2>/dev/null)" ]; then DIRTY=dirty; else DIRTY=clean; fi
    else
        DIRTY=unknown
    fi
fi

info "revision  $REVISION"
info "tree      $DIRTY"
if [ "$DIRTY" != clean ] && { [ "$MODE" = mainnet ] || [ "$MODE" = local ]; }; then
    cat >&2 <<'WARN'

  NOTE: your working tree has uncommitted changes, so this build cannot match
  anything built from a published commit. Verifying a deployment means
  verifying a COMMIT. Run `git stash` or check out a clean tree first, or pass
  --revision <sha> --dirty clean if you know exactly what you are doing.

WARN
fi

# ---------------------------------------------------------------------------
# Build the image. The build args become git:revision / git:dirty metadata
# inside every module, so they are part of the bytes being hashed.
# ---------------------------------------------------------------------------
build_image() {
    info "building the verification image (first run compiles ic-wasm, allow ~10 min)"
    local log
    log="$(mktemp -t cleardeck-verify-build)"
    if ! docker build --progress=plain \
            --build-arg "GIT_REVISION=$REVISION" \
            --build-arg "GIT_DIRTY=$DIRTY" \
            -t "$IMAGE_TAG" "$REPO_ROOT" >"$log" 2>&1; then
        tail -40 "$log" >&2
        die "the verification image failed to build. Full log: $log"
    fi
    rm -f "$log"
}

# Print "name<TAB>sha256" for every canister the image built.
image_hashes() {
    docker run --rm "$IMAGE_TAG" \
        | awk '/^(lobby|history|table_1|table_2|table_3|btc_table_1)[[:space:]]/ {print $1 "\t" $2}' \
        | sort
}

# ---------------------------------------------------------------------------
# --two-paths: the reproducibility check itself. Build the same source at two
# different absolute paths inside the image and compare. This is the exact
# property that was broken: a ~274 KB debug name section carried LLVM
# disambiguators derived from the build directory.
# ---------------------------------------------------------------------------
if [ "$MODE" = two-paths ]; then
    build_image
    info "building the same source at two different absolute paths"
    docker run --rm "$IMAGE_TAG" sh -c '
        set -eu
        rm -rf /build/target /build/.icp
        A=/p1
        B=/a/deliberately/much/longer/absolute/path/for/the/very/same/source
        mkdir -p "$A" "$B"
        cp -r /build/. "$A"/
        cp -r /build/. "$B"/
        for d in "$A" "$B"; do (cd "$d" && icp build lobby history table_1 table_2 table_3 btc_table_1 >/dev/null 2>&1); done
        rc=0
        for n in btc_table_1 history lobby table_1 table_2 table_3; do
            ha=$(sha256sum "$A/.icp/cache/artifacts/$n" | cut -d" " -f1)
            hb=$(sha256sum "$B/.icp/cache/artifacts/$n" | cut -d" " -f1)
            if [ "$ha" = "$hb" ]; then
                printf "  ok       %-14s %s\n" "$n" "$ha"
            else
                printf "  MISMATCH %-14s\n    at %s -> %s\n    at %s -> %s\n" "$n" "$A" "$ha" "$B" "$hb"
                rc=1
            fi
        done
        exit $rc
    '
    info "reproducible: identical bytes from two different build directories"
    exit 0
fi

# ---------------------------------------------------------------------------
# --emit: hand the container-built wasms to the caller. This is how a deploy
# ships the artifact a stranger can reproduce, instead of one built on
# whichever laptop ran the deploy.
# ---------------------------------------------------------------------------
if [ "$MODE" = emit ]; then
    build_image
    mkdir -p "$EMIT_DIR"
    cid="$(docker create "$IMAGE_TAG")"
    trap 'docker rm -f "$cid" >/dev/null 2>&1 || true' EXIT
    for name in "${CANISTERS[@]}"; do
        docker cp "$cid:/build/.icp/cache/artifacts/$name" "$EMIT_DIR/$name.wasm" >/dev/null
        printf '  %-14s %s  %s\n' "$name" \
            "$( (sha256sum "$EMIT_DIR/$name.wasm" 2>/dev/null || shasum -a 256 "$EMIT_DIR/$name.wasm") | cut -d' ' -f1)" \
            "$EMIT_DIR/$name.wasm"
    done
    info "wrote container-built modules to $EMIT_DIR"
    exit 0
fi

# ---------------------------------------------------------------------------
# --mainnet / --local: compare the image's hashes against what is deployed.
# ---------------------------------------------------------------------------
command -v icp >/dev/null 2>&1 || die "icp-cli is required to read deployed module hashes. npm i -g @icp-sdk/icp-cli@1.0.2"

if [ "$MODE" = mainnet ]; then
    ENVIRONMENT=ic
    IDS_FILE="$REPO_ROOT/.icp/data/mappings/ic.ids.json"
else
    ENVIRONMENT=local
    IDS_FILE="$REPO_ROOT/.icp/cache/mappings/local.ids.json"
fi
[ -f "$IDS_FILE" ] || die "no canister id mapping at $IDS_FILE"

ID_FLAG=()
[ -n "$IDENTITY" ] && ID_FLAG=(--identity "$IDENTITY")

canister_id() {
    # Deliberately no jq dependency.
    sed -n "s/.*\"$1\"[[:space:]]*:[[:space:]]*\"\([^\"]*\)\".*/\1/p" "$IDS_FILE" | head -1
}

deployed_hash() {
    icp canister status "$1" -e "$ENVIRONMENT" "${ID_FLAG[@]}" 2>/dev/null \
        | sed -n 's/.*[Mm]odule hash:[[:space:]]*0x\([0-9a-fA-F]\{64\}\).*/\1/p' | head -1
}

deployed_metadata() {
    icp canister metadata "$1" "$2" -e "$ENVIRONMENT" "${ID_FLAG[@]}" 2>/dev/null | tr -d '\r\n' || true
}

build_image

info "hashes produced by the image"
IMAGE_OUT="$(image_hashes)"
echo "$IMAGE_OUT" | sed 's/^/  /' | expand -t 20

echo
info "module hashes reported by the $ENVIRONMENT network"
fail=0
printf '  %-14s %-8s %s\n' CANISTER RESULT DETAIL
for name in "${CANISTERS[@]}"; do
    cid="$(canister_id "$name")"
    if [ -z "$cid" ]; then
        printf '  %-14s %-8s %s\n' "$name" SKIP "no canister id in $(basename "$IDS_FILE")"
        continue
    fi
    built="$(echo "$IMAGE_OUT" | awk -v n="$name" '$1==n {print $2}')"
    live="$(deployed_hash "$cid")"
    if [ -z "$live" ]; then
        printf '  %-14s %-8s %s\n' "$name" UNREAD "no module hash from icp for $cid"
        if [ "$ENVIRONMENT" = ic ]; then
            printf '  %-14s %-8s %s\n' "" "" "read it here instead, no key needed:"
            printf '  %-14s %-8s   https://dashboard.internetcomputer.org/canister/%s\n' "" "" "$cid"
            printf '  %-14s %-8s and compare against built=%s\n' "" "" "$built"
        fi
        fail=1
    elif [ "$built" = "$live" ]; then
        printf '  %-14s %-8s %s\n' "$name" MATCH "$live"
    else
        printf '  %-14s %-8s built=%s\n  %-14s %-8s live =%s\n' "$name" MISMATCH "$built" "" "" "$live"
        fail=1
    fi
    claimed_rev="$(deployed_metadata "$cid" git:revision)"
    claimed_dirty="$(deployed_metadata "$cid" git:dirty)"
    if [ -n "$claimed_rev" ]; then
        note=""
        [ "$claimed_rev" != "$REVISION" ] && note="  <-- differs from the revision you built"
        printf '  %-14s %-8s claims %s (%s)%s\n' "" "" "$claimed_rev" "${claimed_dirty:-?}" "$note"
    fi
done

echo
if [ "$fail" -eq 0 ]; then
    printf '\033[32mVERIFIED\033[0m every deployed module is byte-identical to a build of this source.\n'
else
    cat <<'EOF'
NOT VERIFIED. At least one module does not match. Possible reasons, in the
order they are actually likely:

  1. The deployment is from a different commit than the one you built. Check
     the git:revision each canister claims (printed above), check out that
     commit, and run this again.
  2. The deployment shipped a locally built module instead of a container
     built one. rustc does not produce identical wasm on a macOS host and a
     Linux host even at the same version, which is exactly why the container
     is the reference environment.
  3. The source has changed since deployment.
  4. Something is wrong. Do not deposit.
EOF
    exit 1
fi
