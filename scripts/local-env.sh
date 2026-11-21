#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# karis-ky Escrow — Local Soroban Development Environment
# ─────────────────────────────────────────────────────────────────────────────
#
# Sets up a complete local Soroban standalone network for testing the escrow
# contract without touching any live network. Creates identities, deploys a
# test token, and deploys the escrow contract ready for development.
#
# Usage:
#   source scripts/local-env.sh         # source to keep exports in your shell
#   bash scripts/local-env.sh           # run standalone (exports printed)
#
# Prerequisites:
#   - Rust stable + wasm32-unknown-unknown target
#   - Stellar CLI v22+ (`stellar --version`)
#   - Docker (`docker --version`)
#   - jq (`jq --version`) for JSON parsing
#
# What this script does:
#   1. Checks prerequisites
#   2. Starts a local Soroban validator (Docker container)
#   3. Creates named identities (admin, sme, investor1, investor2, treasury)
#   4. Builds the escrow WASM
#   5. Deploys a test SEP-41 token (native XLM wrapper)
#   6. Deploys the escrow contract
#   7. Exports all needed environment variables
#   8. Prints a summary and example invocation
#
# Schema version: 6
# Interface version: 1
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

# ── Configuration ───────────────────────────────────────────────────────────

NETWORK_NAME="${NETWORK_NAME:-local}"
RPC_URL="${RPC_URL:-http://localhost:8000/soroban/rpc}"
NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
CONTAINER_NAME="${CONTAINER_NAME:-stellar-local}"

WASM_TARGET="wasm32-unknown-unknown"
WASM_PATH="target/${WASM_TARGET}/release/karis_ky_escrow.wasm"

# ── Color helpers ───────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info()    { echo -e "${BLUE}[INFO]${NC} $*"; }
success() { echo -e "${GREEN}[OK]${NC}   $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC} $*"; }
error()   { echo -e "${RED}[ERR]${NC}  $*"; exit 1; }

# ── Step 1: Check prerequisites ─────────────────────────────────────────────

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  karis-ky Escrow — Local Development Environment Setup"
echo "  Schema v6  |  Interface v1"
echo "═══════════════════════════════════════════════════════════════"
echo ""

info "Checking prerequisites..."

command -v cargo   >/dev/null 2>&1 || error "cargo not found. Install Rust: https://rustup.rs"
command -v stellar >/dev/null 2>&1 || error "stellar CLI not found. Install: cargo install --locked stellar-cli --features opt"
command -v docker  >/dev/null 2>&1 || error "docker not found. Install Docker: https://docs.docker.com/get-docker/"
command -v jq      >/dev/null 2>&1 || warn "jq not found — some features will use fallback parsing. Install: apt-get install jq"

STELLAR_VERSION=$(stellar --version 2>/dev/null || echo "unknown")
success "cargo, stellar (${STELLAR_VERSION}), docker ready"

# ── Step 2: Start local Soroban validator ────────────────────────────────────

info "Starting local Soroban standalone validator..."

# Stop any existing container first
stellar container stop local 2>/dev/null || true

# Start the container
stellar container start local

# Wait for the RPC to become ready
info "Waiting for RPC endpoint to become ready..."
for ((attempt = 0; attempt < 30; attempt++)); do
  if curl -s "${RPC_URL}" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

# Register as a named network
stellar network add \
  --rpc-url "${RPC_URL}" \
  --network-passphrase "${NETWORK_PASSPHRASE}" \
  "${NETWORK_NAME}" 2>/dev/null || true

success "Local validator ready at ${RPC_URL}"

# ── Step 3: Create identities ────────────────────────────────────────────────

info "Creating named identities..."

stellar keys generate admin     --network "${NETWORK_NAME}" --fund 2>/dev/null || true
stellar keys generate sme       --network "${NETWORK_NAME}" --fund 2>/dev/null || true
stellar keys generate investor1 --network "${NETWORK_NAME}" --fund 2>/dev/null || true
stellar keys generate investor2 --network "${NETWORK_NAME}" --fund 2>/dev/null || true
stellar keys generate treasury  --network "${NETWORK_NAME}" --fund 2>/dev/null || true

ADMIN=$(stellar keys address admin)
SME=$(stellar keys address sme)
INVESTOR1=$(stellar keys address investor1)
INVESTOR2=$(stellar keys address investor2)
TREASURY=$(stellar keys address treasury)

success "Identities created"
info "  ADMIN:     ${ADMIN}"
info "  SME:       ${SME}"
info "  INVESTOR1: ${INVESTOR1}"
info "  INVESTOR2: ${INVESTOR2}"
info "  TREASURY:  ${TREASURY}"

# ── Step 4: Build the WASM ──────────────────────────────────────────────────

info "Building escrow WASM..."

rustup target add "${WASM_TARGET}" 2>/dev/null || true
cargo build --target "${WASM_TARGET}" --release -p karis_ky_escrow

if [ ! -f "${WASM_PATH}" ]; then
  error "WASM artifact not found at ${WASM_PATH}"
fi

WASM_SIZE=$(du -h "${WASM_PATH}" | cut -f1)
success "WASM built (${WASM_SIZE})"

# ── Step 5: Deploy test token ────────────────────────────────────────────────

info "Deploying test SEP-41 token (native XLM wrapper)..."

TOKEN_ID=$(stellar contract asset deploy \
  --asset native \
  --source admin \
  --network "${NETWORK_NAME}")

success "Test token deployed: ${TOKEN_ID}"

# ── Step 6: Deploy escrow contract ───────────────────────────────────────────

info "Deploying escrow contract..."

CONTRACT_ID=$(stellar contract deploy \
  --wasm "${WASM_PATH}" \
  --source admin \
  --network "${NETWORK_NAME}")

success "Escrow contract deployed: ${CONTRACT_ID}"

# ── Step 7: Verify deployment ────────────────────────────────────────────────

info "Verifying deployment..."

# Check that get_version fails cleanly (contract is deployed but not initialized)
if stellar contract invoke \
  --id "${CONTRACT_ID}" \
  --network "${NETWORK_NAME}" \
  -- get_version 2>/dev/null; then
  warn "get_version succeeded unexpectedly (contract may already be initialized)"
else
  success "Contract deployed and awaiting init (get_version returns error as expected)"
fi

# ── Step 8: Export environment variables ─────────────────────────────────────

# If the script is sourced ($0 differs from caller), export automatically.
# Otherwise, print export commands for the user to copy.

export ADMIN
export SME
export INVESTOR1
export INVESTOR2
export TREASURY
export TOKEN_ID
export CONTRACT_ID
export NETWORK_NAME
export RPC_URL
export NETWORK_PASSPHRASE

if [[ "${BASH_SOURCE[0]}" != "${0}" ]]; then
  echo ""
  success "Environment variables exported to current shell."
  echo ""
  echo "  Source this script to use them:"
  echo "    source scripts/local-env.sh"
else
  echo ""
  echo "────────────────────────────────────────────────────────────────"
  echo "  Copy these exports into your shell session:"
  echo "────────────────────────────────────────────────────────────────"
  echo ""
  echo "export ADMIN=\"${ADMIN}\""
  echo "export SME=\"${SME}\""
  echo "export INVESTOR1=\"${INVESTOR1}\""
  echo "export INVESTOR2=\"${INVESTOR2}\""
  echo "export TREASURY=\"${TREASURY}\""
  echo "export TOKEN_ID=\"${TOKEN_ID}\""
  echo "export CONTRACT_ID=\"${CONTRACT_ID}\""
  echo "export NETWORK_NAME=\"${NETWORK_NAME}\""
  echo "export RPC_URL=\"${RPC_URL}\""
  echo "export NETWORK_PASSPHRASE=\"${NETWORK_PASSPHRASE}\""
  echo ""
fi

# ── Step 9: Print summary ───────────────────────────────────────────────────

echo "═══════════════════════════════════════════════════════════════"
echo "  Local environment ready!"
echo "═══════════════════════════════════════════════════════════════"
echo ""
echo "  Contract ID:  ${CONTRACT_ID}"
echo "  Token ID:     ${TOKEN_ID}"
echo "  Network:      ${NETWORK_NAME} (${RPC_URL})"
echo ""
echo "  Quick start — initialize an escrow:"
echo ""
echo "    stellar contract invoke \\"
echo "      --id \"\${CONTRACT_ID}\" \\"
echo "      --source admin \\"
echo "      --network \"\${NETWORK_NAME}\" \\"
echo "      -- init \\"
echo "      --admin \"\${ADMIN}\" \\"
echo "      --invoice_id \"INV001\" \\"
echo "      --sme_address \"\${SME}\" \\"
echo "      --amount 10000_0000000 \\"
echo "      --yield_bps 800 \\"
echo "      --maturity 0 \\"
echo "      --funding_token \"\${TOKEN_ID}\" \\"
echo "      --registry null \\"
echo "      --treasury \"\${TREASURY}\" \\"
echo "      --yield_tiers null \\"
echo "      --min_contribution null \\"
echo "      --max_unique_investors null"
echo ""
echo "  Next steps:"
echo "    - Read docs/demos/ for the full lifecycle walkthrough"
echo "    - Read docs/escrow-init-parameters.md for parameter guidance"
echo "    - Read docs/escrow-sim-stellar-cli.md for all CLI recipes"
echo ""
echo "  Stop the local validator:"
echo "    stellar container stop local"
echo ""
