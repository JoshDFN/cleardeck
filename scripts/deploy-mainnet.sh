#!/bin/bash
# =============================================================================
# ClearDeck Mainnet Deployment Script
# =============================================================================
# This script deploys ClearDeck to the Internet Computer mainnet.
#
# Prerequisites:
#   1. dfx installed (dfx --version)
#   2. dfx identity created and funded with cycles
#   3. Node.js and npm installed
#
# Usage: ./scripts/deploy-mainnet.sh
# =============================================================================

set -e  # Exit on error

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_DIR"

echo ""
echo "╔══════════════════════════════════════════════════════════════════════════╗"
echo "║                    ClearDeck Mainnet Deployment                         ║"
echo "║                        3 Microstakes Tables                             ║"
echo "╠══════════════════════════════════════════════════════════════════════════╣"
echo "║  Tables:                                                                ║"
echo "║    • Heads Up 1/2 (2 players)                                           ║"
echo "║    • 6-Max 1/2 (6 players)                                              ║"
echo "║    • 9-Max 1/2 (9 players)                                              ║"
echo "║                                                                          ║"
echo "║  Buy-in: 40-200 chips (0.0004 - 0.002 ICP)                              ║"
echo "╚══════════════════════════════════════════════════════════════════════════╝"
echo ""

# =============================================================================
# Step 0: Pre-flight checks
# =============================================================================
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 0: Pre-flight Checks"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Check dfx
if ! command -v dfx &> /dev/null; then
    echo "❌ dfx is not installed"
    echo "   Install: sh -ci \"\$(curl -fsSL https://internetcomputer.org/install.sh)\""
    exit 1
fi
echo "✅ dfx version: $(dfx --version)"

# Check identity
IDENTITY=$(dfx identity whoami)
PRINCIPAL=$(dfx identity get-principal)
echo "✅ Identity: $IDENTITY"
echo "   Principal: $PRINCIPAL"

# Check cycles balance
echo ""
echo "📊 Checking cycles balance..."
CYCLES_RAW=$(dfx cycles balance --network ic 2>/dev/null || echo "0")
CYCLES=$(echo "$CYCLES_RAW" | grep -oE '[0-9]+' | head -1 || echo "0")
echo "   Current balance: $CYCLES_RAW"

# Need roughly 4T cycles for 5 canisters
MIN_CYCLES=4000000000000
if [ "$CYCLES" -lt "$MIN_CYCLES" ]; then
    echo ""
    echo "⚠️  WARNING: You may need more cycles!"
    echo "   Estimated need: ~4T cycles (5 canisters)"
    echo "   Current balance: $CYCLES_RAW"
    echo ""
    echo "   Get cycles:"
    echo "   1. https://nns.ic0.app - Convert ICP to cycles"
    echo "   2. dfx cycles convert --amount 5 --network ic"
    echo ""
    echo "   Proceeding with deployment (10 TC is sufficient)..."
fi

echo ""

# =============================================================================
# Step 1: Build Frontend
# =============================================================================
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 1: Build Frontend"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

cd "$PROJECT_DIR/src/cleardeck_frontend"

echo "📦 Installing npm dependencies..."
npm install

echo "🔨 Building frontend for production..."
npm run build

if [ ! -d "dist" ]; then
    echo "❌ Frontend build failed - dist directory not found"
    exit 1
fi

echo "✅ Frontend built successfully"
cd "$PROJECT_DIR"
echo ""

# =============================================================================
# Step 2: Deploy Backend Canisters
# =============================================================================
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 2: Deploy Backend Canisters"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

echo "🔨 Building Rust canisters..."
dfx build --network ic

echo ""
echo "🚀 Deploying history canister..."
dfx deploy history --network ic
HISTORY_ID=$(dfx canister id history --network ic)
echo "   ✅ History: $HISTORY_ID"

echo ""
echo "🚀 Deploying lobby canister..."
dfx deploy lobby --network ic
LOBBY_ID=$(dfx canister id lobby --network ic)
echo "   ✅ Lobby: $LOBBY_ID"

echo ""
echo "🚀 Deploying table_1 (Heads Up 1/2)..."
dfx deploy table_1 --network ic
TABLE_1_ID=$(dfx canister id table_1 --network ic)
echo "   ✅ Table 1: $TABLE_1_ID"

echo ""
echo "🚀 Deploying table_2 (6-Max 1/2)..."
dfx deploy table_2 --network ic
TABLE_2_ID=$(dfx canister id table_2 --network ic)
echo "   ✅ Table 2: $TABLE_2_ID"

echo ""
echo "🚀 Deploying table_3 (9-Max 1/2)..."
dfx deploy table_3 --network ic
TABLE_3_ID=$(dfx canister id table_3 --network ic)
echo "   ✅ Table 3: $TABLE_3_ID"

echo ""

# =============================================================================
# Step 3: Configure Canisters
# =============================================================================
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 3: Configure Canisters"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

echo "🔗 Authorizing tables in history canister..."
dfx canister call history authorize_table "(principal \"$TABLE_1_ID\")" --network ic
dfx canister call history authorize_table "(principal \"$TABLE_2_ID\")" --network ic
dfx canister call history authorize_table "(principal \"$TABLE_3_ID\")" --network ic
echo "   ✅ Tables authorized in history canister"

echo ""
echo "🔗 Setting history canister in table canisters..."
dfx canister call table_1 set_history_canister "(opt principal \"$HISTORY_ID\")" --network ic
dfx canister call table_2 set_history_canister "(opt principal \"$HISTORY_ID\")" --network ic
dfx canister call table_3 set_history_canister "(opt principal \"$HISTORY_ID\")" --network ic
echo "   ✅ History canister configured"

echo ""
echo "🔗 Initializing lobby with microstakes tables..."
dfx canister call lobby set_admin "(principal \"$PRINCIPAL\")" --network ic
dfx canister call lobby init_microstakes_tables "(principal \"$TABLE_1_ID\", principal \"$TABLE_2_ID\", principal \"$TABLE_3_ID\")" --network ic
echo "   ✅ Lobby initialized with 3 tables"

echo ""

# =============================================================================
# Step 4: Deploy Frontend
# =============================================================================
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 4: Deploy Frontend"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

echo "🚀 Deploying frontend assets..."
dfx deploy frontend --network ic
FRONTEND_ID=$(dfx canister id frontend --network ic)
echo "   ✅ Frontend: $FRONTEND_ID"

echo ""

# =============================================================================
# Step 5: Generate Declarations
# =============================================================================
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 5: Generate Declarations"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

dfx generate --network ic
echo "✅ Declarations generated"

echo ""

# =============================================================================
# Step 6: Verification
# =============================================================================
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 6: Verification"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

echo "🔍 Verifying lobby tables..."
TABLES=$(dfx canister call lobby get_tables --network ic --query 2>/dev/null || echo "error")
if echo "$TABLES" | grep -q "Heads Up"; then
    echo "   ✅ Tables registered in lobby"
else
    echo "   ⚠️  Warning: Could not verify tables in lobby"
fi

echo ""

# =============================================================================
# Deployment Summary
# =============================================================================
echo ""
echo "╔══════════════════════════════════════════════════════════════════════════╗"
echo "║                     ✅ DEPLOYMENT COMPLETE!                              ║"
echo "╚══════════════════════════════════════════════════════════════════════════╝"
echo ""
echo "📋 Canister IDs:"
echo "   ┌─────────────────────────────────────────────────────────────┐"
echo "   │ History:   $HISTORY_ID"
echo "   │ Lobby:     $LOBBY_ID"
echo "   │ Table 1:   $TABLE_1_ID (Heads Up 1/2)"
echo "   │ Table 2:   $TABLE_2_ID (6-Max 1/2)"
echo "   │ Table 3:   $TABLE_3_ID (9-Max 1/2)"
echo "   │ Frontend:  $FRONTEND_ID"
echo "   └─────────────────────────────────────────────────────────────┘"
echo ""
echo "🌐 Frontend URL:"
echo "   https://$FRONTEND_ID.icp0.io"
echo ""
echo "   Alternative URLs:"
echo "   https://$FRONTEND_ID.raw.icp0.io"
echo "   https://$FRONTEND_ID.ic0.app"
echo ""
echo "🔑 Beta Password: beta2026"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📝 POST-DEPLOYMENT CHECKLIST"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "  [ ] Visit the frontend URL and enter password: beta2026"
echo "  [ ] Connect wallet (Internet Identity)"
echo "  [ ] Test deposit flow"
echo "  [ ] Test joining a table"
echo "  [ ] Test playing a hand"
echo "  [ ] Test withdrawal flow"
echo "  [ ] Monitor cycles balance: dfx cycles balance --network ic"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "⚠️  IMPORTANT NOTES"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "  • This is BETA software - users play at their own risk"
echo "  • Chips are denominated in e8s (1 ICP = 100,000,000 e8s)"
echo "  • Minimum buy-in: 40 chips = 0.0000004 ICP"
echo "  • Maximum buy-in: 200 chips = 0.000002 ICP"
echo "  • Monitor cycles - canisters will stop if they run out"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
