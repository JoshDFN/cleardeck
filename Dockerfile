# ClearDeck reproducible build image.
#
# WHAT THIS IS FOR
#   You are about to trust a canister with money. This image lets you check,
#   without trusting us, that the code running in that canister is the code in
#   this repository. You build the image, it prints a sha256 for each canister,
#   and you compare those against the module hashes the Internet Computer
#   reports for the live canisters. If they match, the deployed code is this
#   code. If they do not, something is wrong and you should not deposit.
#
# HOW TO USE IT
#   See the "Verify the Code" section of README.md for the full procedure with
#   expected output. The short version:
#
#     git clone https://github.com/JoshDFN/cleardeck.git && cd cleardeck
#     docker build --build-arg GIT_REVISION=$(git rev-parse HEAD) \
#                  --build-arg GIT_DIRTY=clean -t cleardeck-verify .
#     docker run --rm cleardeck-verify
#
#   Or let scripts/verify-build.sh do all of it and diff the hashes for you.
#
# WHY THE BASE IMAGES ARE PINNED BY DIGEST
#   A tag like `:latest` is a moving target: the image you build from next month
#   is not the image we built from, so the hashes stop matching for a reason
#   that has nothing to do with the source. The previous version of this file
#   used `ghcr.io/dfinity/icp-dev-env:latest`, which (a) drifts, (b) ships
#   rustc 1.83 while this workspace pins 1.90, and (c) could not run `icp` at
#   all -- the binary aborts with a missing libdbus-1.so.3. That Dockerfile
#   never produced a hash, so the README's verification procedure could not
#   have worked for anyone who tried it.
#
# TOOLCHAIN PINNING
#   rustc/cargo   1.90.0   -- from the base image AND from rust-toolchain.toml
#   ic-wasm       0.9.9    -- built here from crates.io with --locked
#   icp-cli       1.0.2    -- must equal the CLI used to deploy
#   The build itself runs recipes/rust-reproducible.hbs, the same recipe a
#   local `icp build` runs, so this image is not a re-implementation of the
#   deploy path: it is the deploy path.
#
# WHY THE PLATFORM IS PINNED TO linux/amd64
#   Measured, not assumed: the same source, the same rustc 1.90.0, the same
#   LLVM 20.1.8, cross-compiled to the same wasm32-unknown-unknown target,
#   produces DIFFERENT wasm depending on the architecture of the machine doing
#   the compiling.
#     x86_64  linux -> 3604035943831a740f7bafea58310e8490e44652893278292149aa6655d9f5f0
#     aarch64 linux -> 86497c39287fb18277f3ea0865db6ef7c05a2113b1823e7d7d4b3df3ea8b1102
#   (raw cargo output for table_canister at one fixed source snapshot.)
#   Without this pin, an x86 verifier and an Apple Silicon verifier building
#   the same commit would get two different hashes and each would conclude the
#   deployment was fraudulent. Pinning costs Apple Silicon users emulation time
#   and buys everyone a single right answer. Do not remove it.
#
#   Same reasoning, different axis: a macOS-native `cargo build` does not match
#   a Linux one either. That is a property of rustc, not of this project, and
#   it is why the container is the reference environment rather than a
#   convenience.
#
#   buildx emits `FromPlatformFlagConstDisallowed` for the two --platform flags
#   below. That lint is aimed at images meant to build natively on any host.
#   This one is the opposite: a constant platform is the whole point. Do not
#   "fix" the warning.

# ---------------------------------------------------------------------------
# Stage 1: fetch the icp-cli native binary. Kept separate so the final image
# does not carry a Node runtime it never uses.
# ---------------------------------------------------------------------------
FROM --platform=linux/amd64 node:22-bookworm-slim@sha256:d649c27dae7ba0137b3cef5dd75baa422c08dc3d9e3fc0c23dfb172dc3cc6436 AS cli

# `command -v icp` finds a JS shim; the real thing is the platform binary in
# the @icp-sdk/icp-cli-linux-{x64,arm64} sub-package. Copy that, so the final
# image needs no Node at all.
ARG ICP_CLI_VERSION=1.0.2
RUN npm i -g --no-fund --no-audit "@icp-sdk/icp-cli@${ICP_CLI_VERSION}" \
    && ICP_BIN="$(find "$(npm root -g)/@icp-sdk" -type f -path '*icp-cli-linux-*/bin/icp' -print -quit)" \
    && test -n "$ICP_BIN" \
    && install -D "$ICP_BIN" /out/icp

# ---------------------------------------------------------------------------
# Stage 2: the build environment.
# ---------------------------------------------------------------------------
FROM --platform=linux/amd64 rust:1.90.0-slim-bookworm@sha256:64232e656c058f4468e8d024e990acff04f0fd5a5c0a88a574dc37773d7325c9

# libdbus-1-3 is a runtime dependency of the icp-cli binary (it reads the OS
# keyring). Without it `icp` exits 127 before doing anything.
# g++/cmake are needed only to compile ic-wasm, which vendors binaryen.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
         libdbus-1-3 ca-certificates g++ cmake make \
    && rm -rf /var/lib/apt/lists/*

COPY --from=cli /out/icp /usr/local/bin/icp
RUN icp --version

# The @dfinity/rust recipe family shells out to ic-wasm. Pin it: a different
# ic-wasm shrinks differently and the module hash moves.
ARG IC_WASM_VERSION=0.9.9
RUN cargo install ic-wasm --version "${IC_WASM_VERSION}" --locked

RUN rustup target add wasm32-unknown-unknown

# Fixed build directory. Combined with the --remap-path-prefix that
# recipes/rust-reproducible.hbs applies to $CARGO_HOME and $(pwd), no absolute
# path from this machine survives into the module.
WORKDIR /build

# Manifest, lockfile, toolchain pin and the vendored build recipe.
COPY icp.yaml Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY recipes ./recipes

# Every crate in the workspace. poker_core carries the shuffle and the hand
# evaluator and all three canister crates depend on it; leaving it out is what
# made the previous image fail to compile from the moment the crate was
# extracted, which silently broke the only verification path the README offers
# a stranger. An independent auditor found that before we did.
COPY src/poker_core ./src/poker_core
COPY src/table_canister ./src/table_canister
COPY src/lobby_canister ./src/lobby_canister
COPY src/history_canister ./src/history_canister

# The revision is an input, not something the build discovers: there is no .git
# in a Docker context. Pass the commit you are verifying. GIT_DIRTY should be
# "clean" for any published commit; it exists so a build from a modified
# working tree cannot masquerade as a build from a commit.
ARG GIT_REVISION=unknown
ARG GIT_DIRTY=unknown
ENV CLEARDECK_GIT_REVISION=${GIT_REVISION}
ENV CLEARDECK_GIT_DIRTY=${GIT_DIRTY}

# Build the six Rust canisters. The frontend asset canister is not built here:
# it is not a wasm module whose hash you compare, and pulling npm dependencies
# would make this image non-reproducible.
RUN icp build lobby history table_1 table_2 table_3 btc_table_1

# Print one sha256 per canister, sorted, in the format verify-build.sh parses
# and a human can eyeball. .icp/cache/artifacts/<name> is the exact byte string
# `icp deploy` installs, so this is the module hash the IC will report.
RUN set -eu; \
    { \
      echo "cleardeck reproducible build"; \
      echo "git:revision  ${CLEARDECK_GIT_REVISION}"; \
      echo "git:dirty     ${CLEARDECK_GIT_DIRTY}"; \
      echo "$(cargo --version)"; \
      echo "ic-wasm       $(ic-wasm --version)"; \
      echo "icp-cli       $(icp --version)"; \
      echo ""; \
      echo "MODULE HASHES (sha256 of the installed wasm)"; \
      for name in btc_table_1 history lobby table_1 table_2 table_3; do \
        printf '%-14s %s\n' "$name" "$(sha256sum ".icp/cache/artifacts/$name" | cut -d' ' -f1)"; \
      done; \
    } > /build/HASHES.txt; \
    cat /build/HASHES.txt

CMD ["cat", "/build/HASHES.txt"]
