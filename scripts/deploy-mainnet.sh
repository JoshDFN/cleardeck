#!/bin/bash
# =============================================================================
# ClearDeck Mainnet Deployment (icp-cli)
# =============================================================================
# Deploys ClearDeck to the Internet Computer mainnet using @icp-sdk/icp-cli.
# Replaces the previous dfx-based script.
#
# ⚠️  THESE CANISTERS HOLD REAL USER FUNDS (ICP + ckBTC).
#     - Backend canisters are deployed with `--mode upgrade` ONLY.
#     - NEVER reinstall a backend canister — reinstall wipes stable state,
#       including user balances.
#     - icp maps canister names -> live mainnet IDs via
#       .icp/data/mappings/ic.ids.json (committed). Verify it lists the IDs in
#       canister_ids.json BEFORE deploying, or icp may CREATE NEW canisters
#       instead of upgrading the live ones.
#
# Prerequisites:
#   1. icp installed              (icp --version ; target 1.0.0 GA)
#   2. ic-wasm + cargo + wasm32 target (the @dfinity/rust recipe needs them)
#   3. jq, node/npm
#   4. A funded deploy identity (pass via IDENTITY=<name>)
#
# Usage:
#   ./scripts/deploy-mainnet.sh                 # upgrade all canisters (current identity)
#   IDENTITY=my-deploy ./scripts/deploy-mainnet.sh
#   CONFIGURE=1 IDENTITY=my-deploy ./scripts/deploy-mainnet.sh   # FIRST deploy only: run wiring
#   SKIP_CONFIRM=1 ./scripts/deploy-mainnet.sh  # non-interactive
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_DIR"

ENVIRONMENT="ic"
IDENTITY="${IDENTITY:-}"        # empty = use the current icp default identity
CONFIGURE="${CONFIGURE:-0}"     # 1 = run first-deploy wiring. DO NOT use on a live upgrade.
SKIP_CONFIRM="${SKIP_CONFIRM:-0}"

# Backend (fund-holding) canisters — UPGRADE ONLY, never reinstall.
BACKEND_CANISTERS=(history lobby table_1 table_2 table_3 btc_table_1)

# --identity flag only if an identity was specified.
ID_FLAG=()
[ -n "$IDENTITY" ] && ID_FLAG=(--identity "$IDENTITY")

echo ""
echo "╔══════════════════════════════════════════════════════════════════════════╗"
echo "║                ClearDeck Mainnet Deployment (icp-cli)                     ║"
echo "║   ⚠  Real funds — backend canisters are UPGRADED, never reinstalled.     ║"
echo "╚══════════════════════════════════════════════════════════════════════════╝"
echo ""

# =============================================================================
# Step 0: Pre-flight checks
# =============================================================================
echo "━━━ STEP 0: Pre-flight checks ━━━"
for tool in icp jq node npm ic-wasm cargo; do
    command -v "$tool" >/dev/null 2>&1 || { echo "❌ $tool is not installed"; exit 1; }
done
echo "✅ icp version: $(icp --version)"

PRINCIPAL="$(icp identity principal ${ID_FLAG[@]+"${ID_FLAG[@]}"} 2>/dev/null || echo 'unknown')"
echo "✅ Deploy principal: $PRINCIPAL"

echo "📊 Cycles balance:"
icp cycles balance -e "$ENVIRONMENT" ${ID_FLAG[@]+"${ID_FLAG[@]}"} || echo "   (could not read balance)"

# Sanity: verify icp knows the live mainnet IDs (else it would create new canisters).
if [ -f .icp/data/mappings/${ENVIRONMENT}.ids.json ]; then
    echo "✅ icp id-mapping present (.icp/data/mappings/${ENVIRONMENT}.ids.json):"
    jq -r 'to_entries[] | "   \(.key) -> \(.value)"' ".icp/data/mappings/${ENVIRONMENT}.ids.json"
else
    echo "❌ .icp/data/mappings/${ENVIRONMENT}.ids.json is MISSING."
    echo "   Without it, 'icp deploy' may CREATE NEW canisters and orphan the live"
    echo "   fund-holding ones. Populate it from canister_ids.json before deploying."
    exit 1
fi

if [ "$SKIP_CONFIRM" != "1" ]; then
    echo ""
    read -r -p "Upgrade the 6 LIVE fund canisters + frontend on mainnet? [type 'upgrade' to proceed] " ans
    [ "$ans" = "upgrade" ] || { echo "Aborted."; exit 1; }
fi
echo ""

# =============================================================================
# Step 1: Build frontend
# =============================================================================
echo "━━━ STEP 1: Build frontend ━━━"
# icp does NOT emit a canister-id .env (unlike dfx). EXPORT the VITE_ IDs the
# bundle needs (lobby + history; table IDs are fetched from the lobby at runtime)
# from the committed live-ID mapping. Exporting (not overwriting .env) preserves
# the operator's existing .env (VITE_BETA_PASSWORD_HASH, etc.); dotenv does not
# override already-exported vars, and vite embeds the REAL IDs.
IDS=".icp/data/mappings/${ENVIRONMENT}.ids.json"
export VITE_CANISTER_ID_LOBBY="$(jq -r '.lobby' "$IDS")"
export VITE_CANISTER_ID_HISTORY="$(jq -r '.history' "$IDS")"
( cd src/cleardeck_frontend && npm install && npm run build )
[ -d src/cleardeck_frontend/dist ] || { echo "❌ frontend build failed (no dist/)"; exit 1; }
grep -rq "$VITE_CANISTER_ID_LOBBY" src/cleardeck_frontend/dist 2>/dev/null \
    || { echo "❌ frontend bundle missing lobby canister ID — aborting"; exit 1; }
echo "✅ Frontend built"
echo ""

# =============================================================================
# Step 2: Deploy backend canisters (UPGRADE ONLY)
# =============================================================================
echo "━━━ STEP 2: Upgrade backend canisters ━━━"
for c in "${BACKEND_CANISTERS[@]}"; do
    echo "🚀 icp deploy $c --mode upgrade"
    icp deploy -e "$ENVIRONMENT" "$c" ${ID_FLAG[@]+"${ID_FLAG[@]}"} --mode upgrade -y
done
echo "✅ Backend canisters upgraded"
echo ""

# Resolve live IDs from canister_ids.json (source of truth for the summary + wiring).
HISTORY_ID="$(jq -r '.history.ic' canister_ids.json)"
LOBBY_ID="$(jq -r '.lobby.ic' canister_ids.json)"
TABLE_1_ID="$(jq -r '.table_1.ic' canister_ids.json)"
TABLE_2_ID="$(jq -r '.table_2.ic' canister_ids.json)"
TABLE_3_ID="$(jq -r '.table_3.ic' canister_ids.json)"
BTC_TABLE_1_ID="$(jq -r '.btc_table_1.ic' canister_ids.json)"
FRONTEND_ID="$(jq -r '.frontend.ic' canister_ids.json)"

# =============================================================================
# Step 3: Configure canisters (FIRST DEPLOY ONLY)
# =============================================================================
# On a live UPGRADE these are already wired and state persists, so re-running
# set_admin / init_microstakes_tables is unnecessary and potentially disruptive.
# Opt in explicitly with CONFIGURE=1 only for a fresh deploy.
if [ "$CONFIGURE" = "1" ]; then
    echo "━━━ STEP 3: Configure canisters (CONFIGURE=1) ━━━"

    echo "🔗 Authorizing tables in history canister..."
    for tid in "$TABLE_1_ID" "$TABLE_2_ID" "$TABLE_3_ID" "$BTC_TABLE_1_ID"; do
        icp canister call history authorize_table "(principal \"$tid\")" -e "$ENVIRONMENT" ${ID_FLAG[@]+"${ID_FLAG[@]}"}
    done

    echo "🔗 Setting history canister in table canisters..."
    for tc in table_1 table_2 table_3 btc_table_1; do
        icp canister call "$tc" set_history_canister "(opt principal \"$HISTORY_ID\")" -e "$ENVIRONMENT" ${ID_FLAG[@]+"${ID_FLAG[@]}"}
    done

    echo "🔗 Initializing lobby..."
    icp canister call lobby set_admin "(principal \"$PRINCIPAL\")" -e "$ENVIRONMENT" ${ID_FLAG[@]+"${ID_FLAG[@]}"}
    icp canister call lobby init_microstakes_tables \
        "(principal \"$TABLE_1_ID\", principal \"$TABLE_2_ID\", principal \"$TABLE_3_ID\")" \
        -e "$ENVIRONMENT" ${ID_FLAG[@]+"${ID_FLAG[@]}"}
    # NOTE: verify whether btc_table_1 needs a separate lobby registration call.
    echo "✅ Configuration complete"
    echo ""
else
    echo "━━━ STEP 3: Skipping configuration (live upgrade). Set CONFIGURE=1 for first deploy. ━━━"
    echo ""
fi

# =============================================================================
# Step 4: Deploy frontend asset canister
# =============================================================================
echo "━━━ STEP 4: Deploy frontend ━━━"
icp deploy -e "$ENVIRONMENT" frontend ${ID_FLAG[@]+"${ID_FLAG[@]}"} --mode upgrade -y
echo "✅ Frontend deployed"
echo ""

# =============================================================================
# Step 5: Verification
# =============================================================================
echo "━━━ STEP 5: Verification ━━━"
if icp canister call lobby get_tables -e "$ENVIRONMENT" --query 2>/dev/null | grep -q "Heads Up"; then
    echo "✅ Tables registered in lobby"
else
    echo "⚠️  Could not verify tables in lobby"
fi
echo ""

# =============================================================================
# Summary
# =============================================================================
echo "╔══════════════════════════════════════════════════════════════════════════╗"
echo "║                       ✅ DEPLOYMENT COMPLETE                              ║"
echo "╚══════════════════════════════════════════════════════════════════════════╝"
echo "   History:     $HISTORY_ID"
echo "   Lobby:       $LOBBY_ID"
echo "   Table 1:     $TABLE_1_ID (Heads-Up 0.01/0.02 ICP)"
echo "   Table 2:     $TABLE_2_ID (6-max 0.05/0.10 ICP)"
echo "   Table 3:     $TABLE_3_ID (9-max 0.10/0.20 ICP)"
echo "   BTC Table 1: $BTC_TABLE_1_ID (Heads-Up 100/200 sats)"
echo "   Frontend:    $FRONTEND_ID"
echo ""
echo "   🌐 https://$FRONTEND_ID.icp0.io"
echo ""
echo "   Monitor cycles: icp cycles balance -e ic"
echo ""
