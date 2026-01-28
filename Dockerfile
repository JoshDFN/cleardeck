# Reproducible build environment for IC canisters
# This allows anyone to verify the deployed WASM matches the source code
#
# Build: docker build -t cleardeck-build .
# Run:   docker run --rm cleardeck-build
#
# The output WASM hashes should match what's deployed on mainnet.
# Check deployed hashes with: dfx canister info <canister-id> --network ic

FROM ghcr.io/dfinity/icp-dev-env:latest

WORKDIR /build

# Copy source files
COPY Cargo.toml Cargo.lock ./
COPY src/table_canister ./src/table_canister
COPY src/lobby_canister ./src/lobby_canister
COPY src/history_canister ./src/history_canister
COPY dfx.json ./

# Build all canisters
RUN dfx build --check

# Output the WASM hashes for verification
RUN echo "=== WASM Module Hashes ===" && \
    echo "Compare these with deployed canisters using:" && \
    echo "  dfx canister info <canister-id> --network ic" && \
    echo "" && \
    for wasm in .dfx/local/canisters/*/*.wasm; do \
        echo "$(basename $(dirname $wasm)): $(sha256sum $wasm | cut -d' ' -f1)"; \
    done
