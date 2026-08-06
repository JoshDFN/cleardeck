#!/usr/bin/env bash
# ClearDeck build verification.
# GitHub: https://github.com/JoshDFN/cleardeck
#
# Answers one question: is the code running in these canisters the code in this
# repository? It rebuilds the canisters and compares the sha256 of each module
# against the module hash the Internet Computer reports for the live canister.
#
#   ./scripts/verify-build.sh --mainnet          verify the live mainnet canisters
#   ./scripts/verify-build.sh --local            verify your local replica
#   ./scripts/verify-build.sh --two-paths        just prove the build is reproducible
#   ./scripts/verify-build.sh --emit <dir>       build and write the wasms to <dir>
#
#   --docker / --host                            force which toolchain rebuilds
#
# TWO BUILDERS, AND WHY THE DEFAULT DIFFERS BY TARGET
#
#   The mainnet fleet is deployed from modules built in the digest-pinned
#   linux/amd64 image, so --mainnet always rebuilds in that image. There is one
#   right answer and every verifier must get it.
#
#   `./scripts/dev.sh local-up` builds with YOUR toolchain on YOUR machine,
#   because making every local bring-up wait on an emulated container build would
#   be absurd. A macOS `cargo build` does not produce the same wasm as a Linux
#   one -- that is a property of rustc, documented in the Dockerfile -- so
#   comparing a container build against a host-built local deployment yields six
#   mismatches that look exactly like tampering and are not. That is what an
#   auditor hit. So --local rebuilds the way the local deployment was built, and
#   says so. `local-up --docker` deploys container-built modules instead, and
#   then `--local --docker` is a full rehearsal of the mainnet check.
#
# WHY THE METADATA COMES OFF THE CANISTER
#
#   Every module carries `git:revision` and `git:dirty` as PUBLIC metadata, and
#   those strings are part of the bytes being hashed. The old script always built
#   with YOUR checked-out revision, so verifying a deployment made one commit ago
#   -- or from a dirty tree -- printed MISMATCH for a reason that had nothing to
#   do with the code. This reads the labels off the canister, builds with exactly
#   those labels, and reports the CODE comparison and the LABEL comparison as two
#   separate facts. Both are printed. Neither is inferred from the other.
#
# Reading the deployed module hash uses `icp canister status`, which issues a
# management-canister call. On mainnet that call is controller-only, so if you
# are not a controller the script will tell you it could not read the hash and
# point you at the public dashboard, which shows the module hash to anyone.
#
# The previous version of this script could not work at all: it drove a
# Dockerfile that did not compile. An independent auditor found that. The version
# after it built the wrong artifact for --local. A second auditor found that.

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

CANISTERS=(lobby history table_1 table_2 table_3 btc_table_1)
IMAGE_TAG="cleardeck-verify"
PROVENANCE_FILE="$REPO_ROOT/.icp/cache/cleardeck-build-provenance.txt"

MODE=""
BUILDER=""
REVISION=""
DIRTY=""
EMIT_DIR=""
IDENTITY="${IDENTITY:-}"

die() { printf '\033[31merror\033[0m %s\n' "$*" >&2; exit 1; }
info() { printf '\033[36m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[33m!\033[0m   %s\n' "$*" >&2; }

usage() {
    sed -n '2,48p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

while [ $# -gt 0 ]; do
    case "$1" in
        --mainnet)    MODE=mainnet ;;
        --local)      MODE=local ;;
        --two-paths)  MODE=two-paths ;;
        --emit)       MODE=emit; EMIT_DIR="${2:-}"; [ -n "$EMIT_DIR" ] || die "--emit needs a directory"; shift ;;
        --docker)     BUILDER=docker ;;
        --host)       BUILDER=host ;;
        --revision)   REVISION="${2:-}"; shift ;;
        --dirty)      DIRTY="${2:-}"; shift ;;
        --identity)   IDENTITY="${2:-}"; shift ;;
        -h|--help)    usage 0 ;;
        *)            die "unknown argument '$1' (try --help)" ;;
    esac
    shift
done

[ -n "$MODE" ] || { echo "Pick a target: --mainnet, --local, --two-paths or --emit <dir>." >&2; echo >&2; usage 2; }

require_docker() {
    command -v docker >/dev/null 2>&1 || die "docker is required for this path. Install Docker and start the daemon."
    docker info >/dev/null 2>&1 || die "the Docker daemon is not running."
}

# ---------------------------------------------------------------------------
# Which builder?
#
# --mainnet, --two-paths and --emit are container-only: they are about the one
# answer every verifier must agree on. --local follows the provenance note that
# `dev.sh local-up` leaves behind, so the verifier rebuilds the way the thing it
# is verifying was built.
# ---------------------------------------------------------------------------
PROVENANCE="(none)"
if [ -r "$PROVENANCE_FILE" ]; then
    PROVENANCE="$(sed -n 's/^builder=//p' "$PROVENANCE_FILE" | head -1)"
    [ -n "$PROVENANCE" ] || PROVENANCE="(unreadable)"
fi

if [ -z "$BUILDER" ]; then
    case "$MODE" in
        mainnet|two-paths|emit) BUILDER=docker ;;
        local)
            case "$PROVENANCE" in
                docker) BUILDER=docker ;;
                *)      BUILDER=host ;;
            esac ;;
    esac
fi

case "$MODE" in
    mainnet|two-paths|emit)
        [ "$BUILDER" = docker ] || die "$MODE is container-only: a host build cannot be compared \
against a deployment anybody else can reproduce. Drop --host." ;;
esac

[ "$BUILDER" = docker ] && require_docker
if [ "$BUILDER" = host ]; then
    command -v cargo >/dev/null 2>&1 || die "cargo is required for a host build (or pass --docker)."
    command -v icp   >/dev/null 2>&1 || die "icp-cli is required for a host build. npm i -g @icp-sdk/icp-cli@1.0.2"
fi

# ---------------------------------------------------------------------------
# What revision does the WORKING TREE say it is? This is only ever used as a
# fallback and as the thing the canister's own claim is compared against.
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

info "working tree   $REVISION ($DIRTY)"
info "builder        $BUILDER$([ "$MODE" = local ] && printf ' (local deployment provenance: %s)' "$PROVENANCE")"

# ---------------------------------------------------------------------------
# Builders. Both print "name<TAB>sha256", sorted, for the SAME six canisters.
# ---------------------------------------------------------------------------
build_image() {
    local rev="$1" dirty="$2" log
    info "building the verification image for $rev ($dirty) — first run compiles ic-wasm, allow ~10 min"
    log="$(mktemp -t cleardeck-verify-build)"
    if ! docker build --progress=plain \
            --build-arg "GIT_REVISION=$rev" \
            --build-arg "GIT_DIRTY=$dirty" \
            -t "$IMAGE_TAG" "$REPO_ROOT" >"$log" 2>&1; then
        tail -40 "$log" >&2
        die "the verification image failed to build. Full log: $log"
    fi
    rm -f "$log"
}

image_hashes() {
    docker run --rm "$IMAGE_TAG" \
        | awk '/^(lobby|history|table_1|table_2|table_3|btc_table_1)[[:space:]]/ {print $1 "\t" $2}' \
        | sort
}

# The host build runs OUTSIDE the repository, in a scratch tree, for three
# reasons: it must not touch `.icp/cache/artifacts` (which is the deployment
# other agents and `dev.sh` are using), the source it compiles is then exactly
# the tracked build inputs and nothing else, and building at a different
# absolute path exercises the --remap-path-prefix work on every run. The cargo
# target directory is kept between runs so this is a rebuild, not a cold build.
HOST_BUILD_DIR="${TMPDIR:-/tmp}/cleardeck-verify-host"
build_host() {
    local rev="$1" dirty="$2" log
    info "building with the host toolchain ($(cargo --version)) for $rev ($dirty)"
    mkdir -p "$HOST_BUILD_DIR/src"
    rm -rf "$HOST_BUILD_DIR/src" "$HOST_BUILD_DIR/recipes" "$HOST_BUILD_DIR/.icp/cache/artifacts"
    mkdir -p "$HOST_BUILD_DIR/src"
    cp -R "$REPO_ROOT/src/poker_core" "$REPO_ROOT/src/table_canister" \
          "$REPO_ROOT/src/lobby_canister" "$REPO_ROOT/src/history_canister" "$HOST_BUILD_DIR/src/"
    cp -R "$REPO_ROOT/recipes" "$HOST_BUILD_DIR/recipes"
    cp "$REPO_ROOT/icp.yaml" "$REPO_ROOT/Cargo.toml" "$REPO_ROOT/Cargo.lock" \
       "$REPO_ROOT/rust-toolchain.toml" "$HOST_BUILD_DIR/"
    log="$(mktemp -t cleardeck-verify-host)"
    if ! ( cd "$HOST_BUILD_DIR" \
           && CLEARDECK_GIT_REVISION="$rev" CLEARDECK_GIT_DIRTY="$dirty" \
              icp build "${CANISTERS[@]}" ) >"$log" 2>&1; then
        tail -40 "$log" >&2
        die "the host build failed. Full log: $log"
    fi
    rm -f "$log"
}

host_hashes() {
    local name
    for name in "${CANISTERS[@]}"; do
        printf '%s\t%s\n' "$name" \
            "$(shasum -a 256 "$HOST_BUILD_DIR/.icp/cache/artifacts/$name" | cut -d' ' -f1)"
    done | sort
}

# Build once for a given (revision, dirty) pair and echo "name<TAB>sha256" lines.
build_and_hash() {
    if [ "$BUILDER" = docker ]; then
        build_image "$1" "$2" >&2
        image_hashes
    else
        build_host "$1" "$2" >&2
        host_hashes
    fi
}

# ---------------------------------------------------------------------------
# --two-paths: the reproducibility check itself. Build the same source at two
# different absolute paths inside the image and compare. This is the exact
# property that was broken: a ~274 KB debug name section carried LLVM
# disambiguators derived from the build directory.
# ---------------------------------------------------------------------------
if [ "$MODE" = two-paths ]; then
    build_image "$REVISION" "$DIRTY"
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
    build_image "$REVISION" "$DIRTY"
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
# --mainnet / --local: compare a rebuild against what is deployed.
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

# ---------------------------------------------------------------------------
# Step 1. Read what is deployed: id, module hash, and the labels it claims.
# ---------------------------------------------------------------------------
info "reading the $ENVIRONMENT canisters"
DEPLOYED=()      # name|cid|live_hash|claimed_rev|claimed_dirty
LABELS=()        # unique "rev|dirty" pairs to build
for name in "${CANISTERS[@]}"; do
    cid="$(canister_id "$name")"
    if [ -z "$cid" ]; then
        DEPLOYED+=("$name||||")
        continue
    fi
    live="$(deployed_hash "$cid")"
    crev="$(deployed_metadata "$cid" git:revision)"
    cdirty="$(deployed_metadata "$cid" git:dirty)"
    # A module with no labels can only be checked against the tree you have.
    [ -n "$crev" ]   || crev="$REVISION"
    [ -n "$cdirty" ] || cdirty="$DIRTY"
    DEPLOYED+=("$name|$cid|$live|$crev|$cdirty")
    pair="$crev|$cdirty"
    case " ${LABELS[*]-} " in *" $pair "*) ;; *) LABELS+=("$pair") ;; esac
    printf '  %-14s %-30s claims %s (%s)\n' "$name" "$cid" "${crev:0:12}" "$cdirty"
done

[ "${#LABELS[@]}" -gt 0 ] || die "no canister in $(basename "$IDS_FILE") could be read"

if [ "${#LABELS[@]}" -gt 1 ]; then
    warn "the fleet does not agree on one revision: ${#LABELS[@]} distinct (revision, dirty) labels."
    warn "each is rebuilt separately below. A fleet at mixed revisions is itself worth explaining."
fi

# ---------------------------------------------------------------------------
# Step 2. Rebuild once per distinct label, with THAT label baked in.
# ---------------------------------------------------------------------------
BUILT=()   # "rev|dirty|name|sha"
for pair in "${LABELS[@]}"; do
    rev="${pair%%|*}"; dty="${pair##*|}"
    while IFS=$'\t' read -r bname bsha; do
        [ -n "$bname" ] || continue
        BUILT+=("$rev|$dty|$bname|$bsha")
    done <<< "$(build_and_hash "$rev" "$dty")"
done

built_hash() { # $1 rev  $2 dirty  $3 name
    local e
    for e in "${BUILT[@]}"; do
        case "$e" in "$1|$2|$3|"*) printf '%s' "${e##*|}"; return 0 ;; esac
    done
    return 1
}

# ---------------------------------------------------------------------------
# Step 3. Compare. CODE and LABEL are reported as two separate facts.
# ---------------------------------------------------------------------------
echo
info "module hashes reported by the $ENVIRONMENT network"
code_fail=0
label_notes=()
printf '  %-14s %-8s %s\n' CANISTER RESULT DETAIL
for entry in "${DEPLOYED[@]}"; do
    IFS='|' read -r name cid live crev cdirty <<< "$entry"
    if [ -z "$cid" ]; then
        printf '  %-14s %-8s %s\n' "$name" SKIP "no canister id in $(basename "$IDS_FILE")"
        continue
    fi
    built="$(built_hash "$crev" "$cdirty" "$name" || true)"
    if [ -z "$live" ]; then
        printf '  %-14s %-8s %s\n' "$name" UNREAD "no module hash from icp for $cid"
        if [ "$ENVIRONMENT" = ic ]; then
            printf '  %-14s %-8s %s\n' "" "" "read it here instead, no key needed:"
            printf '  %-14s %-8s   https://dashboard.internetcomputer.org/canister/%s\n' "" "" "$cid"
            printf '  %-14s %-8s and compare against built=%s\n' "" "" "$built"
        fi
        code_fail=1
    elif [ "$built" = "$live" ]; then
        printf '  %-14s %-8s %s\n' "$name" MATCH "$live"
    else
        printf '  %-14s %-8s built=%s\n  %-14s %-8s live =%s\n' "$name" MISMATCH "$built" "" "" "$live"
        code_fail=1
    fi
    if [ "$crev" != "$REVISION" ]; then
        label_notes+=("$name claims $crev, which is NOT the revision your tree is on ($REVISION)")
    fi
    if [ "$cdirty" != clean ]; then
        label_notes+=("$name was built from a tree marked '$cdirty', so no commit in this repository reproduces it")
    fi
done

# ---------------------------------------------------------------------------
# Step 4. The verdict, and an explicit statement of its scope. Both halves are
# mandatory: a VERIFIED with no scope is how "the code is the code in this repo"
# became a claim nobody could check.
# ---------------------------------------------------------------------------
echo
if [ "$code_fail" -eq 0 ]; then
    printf '\033[32mVERIFIED\033[0m every deployed module is byte-identical to a build of this source.\n'
else
    cat <<'EOF'
NOT VERIFIED. At least one module does not match. Possible reasons, in the
order they are actually likely:

  1. The deployment is older than your working tree. The labels above say which
     revision each canister claims; if that is not your HEAD, the SOURCE has
     moved since the deploy. Check out that commit, or redeploy, and re-run.
  2. The deployment was built with a different toolchain than the one this run
     used. See the builder line at the top: a macOS host build and a linux/amd64
     container build of the same source are NOT byte-identical, by design.
  3. The source has changed since deployment.
  4. Something is wrong. Do not deposit.
EOF
fi

echo
info "what this run does and does not establish"
if [ "$BUILDER" = docker ]; then
    cat <<EOF
  Builder: the digest-pinned linux/amd64 image, which is the reference
  environment every verifier can reproduce. A MATCH here means the deployed
  bytes are the bytes anyone else building this source gets.
EOF
else
    cat <<EOF
  Builder: YOUR machine's toolchain ($(cargo --version)), because that is how
  the local deployment under test was built. A MATCH here proves the local
  replica is running a build of THIS source tree, with the recipe in
  recipes/rust-reproducible.hbs, and that the metadata pipeline works end to
  end. It does NOT establish cross-machine reproducibility: rustc does not emit
  identical wasm on macOS and Linux. For that claim run
  '--two-paths' (container, path independence) or bring the stack up with
  './scripts/dev.sh local-up --docker' and re-run with '--local --docker'.
EOF
fi
cat <<'EOF'

  The labels: `git:revision` and `git:dirty` are strings stamped in at build
  time. They are read off each canister above and are what this run built with,
  so a stale or dishonest label cannot turn a code mismatch into a match -- it
  can only tell you which source to compare. A canister can claim any revision
  it likes; what it cannot do is claim a revision whose build produces its hash.
EOF

if [ "${#label_notes[@]}" -gt 0 ]; then
    echo
    warn "label findings (separate from the code comparison above):"
    for n in "${label_notes[@]}"; do warn "  $n"; done
    if [ "$ENVIRONMENT" = ic ]; then
        warn "  a mainnet deployment that is not reproducible from a published commit is"
        warn "  unverifiable by anyone but the machine that built it. Treat it as unverified."
    fi
fi

[ "$code_fail" -eq 0 ] || exit 1
