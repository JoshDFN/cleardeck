#!/bin/bash
# GitHub: https://github.com/JoshDFN/cleardeck
# Verify that the deployed mainnet canisters match the source code.
#
# 1. Reads the deployed module hash of each backend canister from mainnet (icp).
# 2. Builds from source in a reproducible Docker environment.
# 3. Compares the WASM hashes.
#
# Reading `icp canister status` requires the caller to be a controller of the
# target canister. Pass IDENTITY=<name> if your default identity isn't a controller.
#
# Usage: ./scripts/verify-build.sh

set -euo pipefail

ENVIRONMENT="ic"
IDENTITY="${IDENTITY:-}"
ID_FLAG=()
[ -n "$IDENTITY" ] && ID_FLAG=(--identity "$IDENTITY")

echo "=== ClearDeck Build Verification (icp-cli) ==="
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

echo "Step 1: Reading deployed module hashes from mainnet..."
echo ""

declare -A DEPLOYED_HASHES
for name in "${!CANISTERS[@]}"; do
    id="${CANISTERS[$name]}"
    # icp canister status prints a "Module hash" line; grab the hash token.
    hash=$(icp canister status "$id" -e "$ENVIRONMENT" "${ID_FLAG[@]}" 2>/dev/null \
        | grep -iE 'module hash|module_hash' | grep -oE '0x[0-9a-fA-F]+|[0-9a-fA-F]{64}' | head -1 || echo "error")
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
BUILD_OUTPUT=$(docker run --rm cleardeck-verify 2>&1 | tail -20)
echo "$BUILD_OUTPUT"

echo ""
echo "=== Verification Summary ==="
echo ""
echo "If the hashes match, the deployed code is verified to match the source."
echo "If they don't match, either:"
echo "  - The source has changed since deployment"
echo "  - The build environment differs (toolchain/recipe versions)"
echo "  - Something suspicious is going on"
echo ""
echo "NOTE: confirm the exact 'Module hash' field name from \`icp canister status\`"
echo "      output on first run and adjust the grep above if needed."
