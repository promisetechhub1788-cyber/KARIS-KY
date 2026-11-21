#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# karis-ky Escrow — Deployer Script (Config-driven)
# ─────────────────────────────────────────────────────────────────────────────
#
# Deploys the escrow contract with environment-specific configuration.
# Reads from a .env file or environment variables.
#
# Usage:
#   bash scripts/deploy.sh                 # uses default .env or env vars
#   bash scripts/deploy.sh --env .env.test  # uses specified env file
#   bash scripts/deploy.sh --dry-run        # print resolved plan and exit
#
# Expected environment variables (see .env.example below):
#   STELLAR_NETWORK      — network name (default: local)
#   SOROBAN_RPC_URL      — Soroban RPC endpoint
#   SOURCE_SECRET        — deployer Stellar secret key
#   DEPLOYER_ADDRESS     — deployer address (G...)
#
# Schema version: 6
# Interface version: 1
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

info()    { echo -e "${BLUE}[INFO]${NC} $*"; }
success() { echo -e "${GREEN}[OK]${NC}   $*"; }
error()   { echo -e "${RED}[ERR]${NC}  $*"; exit 1; }

# ── Parse arguments ─────────────────────────────────────────────────────────

ENV_FILE=".env"
DRY_RUN="false"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --env) ENV_FILE="$2"; shift 2 ;;
    --dry-run) DRY_RUN="true"; shift ;;
    *) error "Unknown argument: $1" ;;
  esac
done

# ── Load environment ────────────────────────────────────────────────────────

if [ -f "${ENV_FILE}" ]; then
  info "Loading config from ${ENV_FILE}"
  set -a
  # shellcheck disable=SC1090
  source "${ENV_FILE}"
  set +a
fi

# Defaults
STELLAR_NETWORK="${STELLAR_NETWORK:-local}"
SOROBAN_RPC_URL="${SOROBAN_RPC_URL:-http://localhost:8000/soroban/rpc}"
NETWORK_PASSPHRASE="${NETWORK_PASSPHRASE:-Standalone Network ; February 2017}"
SOURCE_SECRET="${SOURCE_SECRET:-}"
DEPLOYER_ADDRESS="${DEPLOYER_ADDRESS:-}"
WASM_TARGET="${WASM_TARGET:-wasm32-unknown-unknown}"
WASM_PATH="${WASM_PATH:-target/${WASM_TARGET}/release/karis_ky_escrow.wasm}"

# ── Validate ────────────────────────────────────────────────────────────────

if [ -z "${SOURCE_SECRET}" ]; then
  error "SOURCE_SECRET is required. Set it in ${ENV_FILE} or as an env variable."
fi

# ── Dry-run: print resolved plan and exit ───────────────────────────────────

if [ "${DRY_RUN}" = "true" ]; then
  info "Dry-run mode: no deployment will be performed."
  echo ""
  echo "Resolved deployment plan:"
  echo "  env file:           ${ENV_FILE}"
  echo "  network:            ${STELLAR_NETWORK}"
  echo "  rpc url:            ${SOROBAN_RPC_URL}"
  echo "  network passphrase: ${NETWORK_PASSPHRASE}"
  echo "  deployer address:   ${DEPLOYER_ADDRESS:-<unset>}"
  echo "  source secret:      <redacted>"
  echo "  wasm target:        ${WASM_TARGET}"
  echo "  wasm path:          ${WASM_PATH}"
  if [ -f "${WASM_PATH}" ]; then
    WASM_SIZE=$(du -h "${WASM_PATH}" | cut -f1)
    WASM_HASH=$(sha256sum "${WASM_PATH}" | cut -d' ' -f1)
    echo "  wasm size:          ${WASM_SIZE}"
    echo "  wasm sha256:        ${WASM_HASH}"
  else
    echo "  wasm size:          <not built>"
    echo "  wasm sha256:        <not built>"
  fi
  echo "  init args:          (none)"
  echo ""
  success "Dry-run complete. Exiting without deploying."
  exit 0
fi

# ── Build WASM if needed ────────────────────────────────────────────────────

if [ ! -f "${WASM_PATH}" ]; then
  info "WASM not found at ${WASM_PATH}. Building..."
  rustup target add "${WASM_TARGET}" 2>/dev/null || true
  cargo build --target "${WASM_TARGET}" --release -p karis_ky_escrow
fi

WASM_SIZE=$(du -h "${WASM_PATH}" | cut -f1)
success "WASM ready (${WASM_SIZE})"

# ── Deploy ──────────────────────────────────────────────────────────────────

info "Deploying to ${STELLAR_NETWORK} network (${SOROBAN_RPC_URL})..."

CONTRACT_ID=$(stellar contract deploy \
  --wasm "${WASM_PATH}" \
  --source-account "${DEPLOYER_ADDRESS}" \
  --secret-key "${SOURCE_SECRET}" \
  --rpc-url "${SOROBAN_RPC_URL}" \
  --network-passphrase "${NETWORK_PASSPHRASE}" \
  --network "${STELLAR_NETWORK}" 2>/dev/null || \
  stellar contract deploy \
    --wasm "${WASM_PATH}" \
    --source-account "${DEPLOYER_ADDRESS}" \
    --secret-key "${SOURCE_SECRET}" \
    --rpc-url "${SOROBAN_RPC_URL}" \
    --network-passphrase "${NETWORK_PASSPHRASE}")

success "Deployed! Contract ID: ${CONTRACT_ID}"
echo ""
echo "export CONTRACT_ID=\"${CONTRACT_ID}\""
