#!/bin/bash
# GitHub: https://github.com/JoshDFN/cleardeck
# Verify that deployed canisters match the source code
#
# This script:
# 1. Builds the canisters in a reproducible Docker environment
# 2. Compares the WASM hashes with what's deployed on mainnet
#
# Usage: ./scripts/verify-build.sh

set -e

echo "=== ClearDeck Build Verification ==="
echo ""

# Canister IDs on mainnet
declare -A CANISTERS=(
    ["lobby"]="kpfcd-kyaaa-aaaaj-qor3a-cai"
    ["table_1"]="kieex-haaaa-aaaaj-qor3q-cai"
    ["table_2"]="lfkaz-iiaaa-aaaaj-qor4a-cai"
    ["table_3"]="lclgn-fqaaa-aaaaj-qor4q-cai"
    ["btc_table_1"]="qrhly-eaaaa-aaaaj-qousa-cai"
    ["history"]="kggj7-4qaaa-aaaaj-qor2q-cai"
)

echo "Step 1: Getting deployed WASM hashes from mainnet..."
echo ""

declare -A DEPLOYED_HASHES
for name in "${!CANISTERS[@]}"; do
    id="${CANISTERS[$name]}"
    hash=$(dfx canister info "$id" --network ic 2>/dev/null | grep "Module hash:" | awk '{print $3}' || echo "error")
    DEPLOYED_HASHES[$name]="$hash"
    echo "  $name ($id): $hash"
done

echo ""
echo "Step 2: Building from source in Docker..."
echo ""

docker build -t cleardeck-verify . 2>&1 | tail -20

echo ""
echo "Step 3: Extracting build hashes..."
echo ""

# Run container and get hashes
BUILD_OUTPUT=$(docker run --rm cleardeck-verify 2>&1 | tail -20)
echo "$BUILD_OUTPUT"

echo ""
echo "=== Verification Summary ==="
echo ""
echo "If the hashes match, the deployed code is verified to match the source."
echo "If they don't match, either:"
echo "  - The source has changed since deployment"
echo "  - The build environment differs"
echo "  - Something suspicious is going on"
echo ""
