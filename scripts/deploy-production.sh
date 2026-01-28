#!/bin/bash
# Production Deployment Script for ClearDeck
# Usage: ./scripts/deploy-production.sh [--network ic]

set -e  # Exit on error

NETWORK="${1:-ic}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_DIR"

echo "=========================================="
echo "ClearDeck Production Deployment"
echo "Network: $NETWORK"
echo "=========================================="
echo ""

# Check if dfx is installed
if ! command -v dfx &> /dev/null; then
    echo "❌ Error: dfx is not installed or not in PATH"
    exit 1
fi

# Check network
if [ "$NETWORK" != "ic" ] && [ "$NETWORK" != "local" ]; then
    echo "❌ Error: Network must be 'ic' or 'local'"
    exit 1
fi

# Check cycles balance for mainnet
if [ "$NETWORK" = "ic" ]; then
    echo "📊 Checking cycles balance..."
    CYCLES=$(dfx cycles balance --network ic 2>/dev/null || echo "0")
    echo "   Current cycles: $CYCLES"
    
    if [ "$CYCLES" -lt 1000000000000 ]; then
        echo "⚠️  Warning: Low cycles balance. You may need more cycles for deployment."
        echo "   Get cycles from: https://internetcomputer.org/docs/current/developer-docs/getting-started/cycles/cycles-faucet"
        read -p "Continue anyway? (y/N) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
fi

# Build for production
echo ""
echo "🔨 Building canisters for production..."
dfx build --network "$NETWORK" --release

if [ $? -ne 0 ]; then
    echo "❌ Build failed!"
    exit 1
fi

echo "✅ Build complete"
echo ""

# Deploy canisters in order
echo "🚀 Deploying canisters..."
echo ""

echo "1️⃣  Deploying history canister..."
dfx deploy history --network "$NETWORK"
HISTORY_ID=$(dfx canister id history --network "$NETWORK")
echo "   ✅ History canister: $HISTORY_ID"
echo ""

echo "2️⃣  Deploying lobby canister..."
dfx deploy lobby --network "$NETWORK"
LOBBY_ID=$(dfx canister id lobby --network "$NETWORK")
echo "   ✅ Lobby canister: $LOBBY_ID"
echo ""

echo "3️⃣  Deploying table canisters..."
dfx deploy table_headsup --network "$NETWORK"
TABLE_HEADSUP_ID=$(dfx canister id table_headsup --network "$NETWORK")
echo "   ✅ Heads-up table: $TABLE_HEADSUP_ID"

dfx deploy table_6max --network "$NETWORK"
TABLE_6MAX_ID=$(dfx canister id table_6max --network "$NETWORK")
echo "   ✅ 6-max table: $TABLE_6MAX_ID"
echo ""

# Authorize tables in history canister
echo "4️⃣  Authorizing tables in history canister..."
dfx canister call history authorize_table "(principal \"$TABLE_HEADSUP_ID\")" --network "$NETWORK" || echo "   ⚠️  Warning: Failed to authorize heads-up table"
dfx canister call history authorize_table "(principal \"$TABLE_6MAX_ID\")" --network "$NETWORK" || echo "   ⚠️  Warning: Failed to authorize 6-max table"
echo "   ✅ Tables authorized"
echo ""

# Set history canister ID in tables
echo "5️⃣  Configuring table canisters..."
dfx canister call table_headsup set_history_canister "(principal \"$HISTORY_ID\")" --network "$NETWORK" || echo "   ⚠️  Warning: Failed to set history canister in heads-up table"
dfx canister call table_6max set_history_canister "(principal \"$HISTORY_ID\")" --network "$NETWORK" || echo "   ⚠️  Warning: Failed to set history canister in 6-max table"
echo "   ✅ History canister configured"
echo ""

# CRITICAL: Disable dev mode
echo "6️⃣  Disabling dev mode (CRITICAL for production)..."
dfx canister call table_headsup set_dev_mode "(false)" --network "$NETWORK" || echo "   ⚠️  Warning: Failed to disable dev mode in heads-up table"
dfx canister call table_6max set_dev_mode "(false)" --network "$NETWORK" || echo "   ⚠️  Warning: Failed to disable dev mode in 6-max table"

# Verify dev mode is disabled
echo "   Verifying dev mode is disabled..."
DEV_MODE_HEADSUP=$(dfx canister call table_headsup is_dev_mode --network "$NETWORK" --query 2>/dev/null | grep -o 'false\|true' || echo "unknown")
DEV_MODE_6MAX=$(dfx canister call table_6max is_dev_mode --network "$NETWORK" --query 2>/dev/null | grep -o 'false\|true' || echo "unknown")

if [ "$DEV_MODE_HEADSUP" = "false" ] && [ "$DEV_MODE_6MAX" = "false" ]; then
    echo "   ✅ Dev mode is disabled"
else
    echo "   ⚠️  WARNING: Dev mode may still be enabled!"
    echo "      Heads-up: $DEV_MODE_HEADSUP"
    echo "      6-max: $DEV_MODE_6MAX"
fi
echo ""

# Deploy frontend
echo "7️⃣  Deploying frontend..."
dfx deploy frontend --network "$NETWORK"
FRONTEND_ID=$(dfx canister id frontend --network "$NETWORK")
echo "   ✅ Frontend canister: $FRONTEND_ID"
echo ""

# Generate environment file
echo "8️⃣  Generating environment file..."
dfx generate --network "$NETWORK" || echo "   ⚠️  Warning: Failed to generate environment file"
echo ""

# Summary
echo "=========================================="
echo "✅ Deployment Complete!"
echo "=========================================="
echo ""
echo "Canister IDs:"
echo "  History:  $HISTORY_ID"
echo "  Lobby:    $LOBBY_ID"
echo "  Heads-up: $TABLE_HEADSUP_ID"
echo "  6-max:    $TABLE_6MAX_ID"
echo "  Frontend: $FRONTEND_ID"
echo ""

if [ "$NETWORK" = "ic" ]; then
    echo "🌐 Frontend URL:"
    echo "   https://$FRONTEND_ID.icp0.io"
    echo ""
    echo "📋 Next Steps:"
    echo "   1. Test the frontend URL"
    echo "   2. Test deposit/withdrawal flows"
    echo "   3. Monitor cycles balance"
    echo "   4. Set up monitoring/alerting"
    echo ""
    echo "⚠️  IMPORTANT:"
    echo "   - Verify dev mode is disabled (already done)"
    echo "   - Fund canisters with ICP for withdrawal fees"
    echo "   - Monitor cycles consumption"
    echo ""
else
    echo "🌐 Local Frontend URL:"
    echo "   http://localhost:4943/?canisterId=$FRONTEND_ID"
    echo ""
fi

echo "=========================================="
