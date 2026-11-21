#!/usr/bin/env bash
#
# canary-smoke-test.sh
#
# Read-only liveness and correctness smoke test for a deployed canary
# contract. Calls three read-only endpoints and asserts expected values:
#
#   - get_version        -> returns a non-empty version string
#   - get_escrow_health  -> returns a healthy status
#   - get_escrow         -> returns a well-formed escrow record for a known id
#
# The script is idempotent: it only performs read-only calls and can be run
# any number of times against the same contract without side effects.
#
# Usage:
#   scripts/canary-smoke-test.sh <CONTRACT_ID> [ESCROW_ID]
#
# Environment:
#   SOROBAN_RPC_URL   RPC endpoint to query (default: testnet)
#   SOROBAN_NETWORK   Network passphrase name (default: testnet)
#   CANARY_SKIP       When set to "1"/"true", skip the smoke test (exit 0)
#
# Exit codes:
#   0  all checks passed (or skipped)
#   1  a check failed
#   2  usage / missing dependency error

set -euo pipefail

CONTRACT_ID="${1:-}"
ESCROW_ID="${2:-0}"

SOROBAN_RPC_URL="${SOROBAN_RPC_URL:-https://soroban-testnet.stellar.org}"
SOROBAN_NETWORK="${SOROBAN_NETWORK:-testnet}"

# Optional skip (used by PR builds that do not deploy a canary).
case "${CANARY_SKIP:-}" in
  1|true|TRUE|yes|YES)
    echo "canary-smoke-test: CANARY_SKIP set, skipping smoke test."
    exit 0
    ;;
esac

if [ -z "${CONTRACT_ID}" ]; then
  echo "usage: $0 <CONTRACT_ID> [ESCROW_ID]" >&2
  exit 2
fi

if ! command -v soroban >/dev/null 2>&1; then
  echo "canary-smoke-test: 'soroban' CLI not found in PATH" >&2
  exit 2
fi

failures=0

# invoke_read <function> [args...]
# Performs a read-only contract invocation and prints the result to stdout.
invoke_read() {
  local fn="$1"; shift
  soroban contract invoke \
    --id "${CONTRACT_ID}" \
    --rpc-url "${SOROBAN_RPC_URL}" \
    --network "${SOROBAN_NETWORK}" \
    -- "${fn}" "$@"
}

check() {
  local name="$1"
  shift
  if "$@"; then
    echo "  [PASS] ${name}"
  else
    echo "  [FAIL] ${name}" >&2
    failures=$((failures + 1))
  fi
}

# --- get_version ---------------------------------------------------------
check_get_version() {
  local out
  out="$(invoke_read get_version 2>/dev/null || true)"
  # Expect a non-empty version string (quoted or bare).
  [ -n "${out}" ] && [ "${out}" != '""' ]
}

# --- get_escrow_health ---------------------------------------------------
check_get_escrow_health() {
  local out
  out="$(invoke_read get_escrow_health 2>/dev/null || true)"
  # Expect a healthy status; accept boolean true or a status string.
  case "${out}" in
    *true*|*healthy*|*Healthy*|*HEALTHY*|*ok*|*OK*) return 0 ;;
    *) return 1 ;;
  esac
}

# --- get_escrow ----------------------------------------------------------
check_get_escrow() {
  local out
  out="$(invoke_read get_escrow --escrow_id "${ESCROW_ID}" 2>/dev/null || true)"
  # Expect a non-empty, well-formed record (object/struct output).
  [ -n "${out}" ] && [ "${out}" != "null" ] && [ "${out}" != '""' ]
}

echo "canary-smoke-test: contract=${CONTRACT_ID} escrow_id=${ESCROW_ID}"
echo "canary-smoke-test: rpc=${SOROBAN_RPC_URL} network=${SOROBAN_NETWORK}"

check "get_version" check_get_version
check "get_escrow_health" check_get_escrow_health
check "get_escrow" check_get_escrow

if [ "${failures}" -ne 0 ]; then
  echo "canary-smoke-test: ${failures} check(s) failed." >&2
  exit 1
fi

echo "canary-smoke-test: all checks passed."
