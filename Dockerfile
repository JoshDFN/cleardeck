# Reproducible build environment for IC canisters (icp-cli)
# Lets anyone verify the deployed WASM matches the source code.
#
# Build: docker build -t cleardeck-build .
# Run:   docker run --rm cleardeck-build
#
# The output WASM hashes should match what's deployed on mainnet.
# Check deployed hashes with: icp canister status <canister-id> -e ic
#
# NOTE: deploys now use `icp build` (the @dfinity/rust recipe: cargo build +
#       ic-wasm shrink + candid metadata), NOT `dfx build`. The first icp deploy
#       re-establishes the verified module-hash baseline; hashes built here will
#       only match canisters deployed via the same recipe/CLI version.

FROM ghcr.io/dfinity/icp-dev-env:latest

WORKDIR /build

# Pin the CLI used for the reproducible build (must match the deploy CLI).
RUN npm i -g @icp-sdk/icp-cli@1.0.0

# Copy the manifest + Rust sources (the @dfinity/rust recipe builds from Cargo).
COPY icp.yaml Cargo.toml Cargo.lock ./
# poker_core carries the shuffle and the evaluator and every canister depends on
# it. Omitting it made this image fail to compile from the moment the crate was
# extracted, which silently broke the only build-verification path the README
# offers a stranger. An independent auditor found it before we did.
COPY src/poker_core ./src/poker_core
COPY src/table_canister ./src/table_canister
COPY src/lobby_canister ./src/lobby_canister
COPY src/history_canister ./src/history_canister

# Build all canisters via the icp recipe (cargo + ic-wasm shrink + candid metadata).
RUN icp build -e ic

# Output the WASM hashes for verification. The deployed module hash corresponds
# to the post-shrink wasm produced by the recipe; print every produced wasm so
# the matching artifact can be identified.
RUN echo "=== WASM Module Hashes ===" && \
    echo "Compare with deployed canisters using: icp canister status <id> -e ic" && \
    echo "" && \
    find . -name '*.wasm' \( -path '*release*' -o -path '*.icp*' \) ! -path '*/deps/*' 2>/dev/null \
        | sort -u \
        | while read -r wasm; do \
            echo "$(basename "$wasm"): $(sha256sum "$wasm" | cut -d' ' -f1)"; \
          done
