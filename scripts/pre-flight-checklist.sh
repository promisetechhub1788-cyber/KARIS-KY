#!/usr/bin/env bash
#
# Pre-flight checklist for a Liquifact escrow deployment.
#
# CI validates that the references in this script are current: every
# `stellar contract invoke` method used below must exist as a public
# entrypoint in escrow/src/lib.rs, and the required commands (e.g. the
# `stellar` CLI) must be available. See the "Pre-flight checklist
# validation" section in the CI workflow / docs for details. If you add
# or rename an entrypoint, update the `start_check` calls below so this
# checklist does not go stale.

set -u

SCRIPT_NAME="$(basename "$0")"
readonly SCRIPT_NAME
readonly NETWORK="${STELLAR_NETWORK:-local}"
readonly CONTRACT_ID="${CONTRACT_ID:-${LIQUIFACT_CONTRACT_ID:-}}"
readonly CHECK_TIMEOUT_SECS="${CHECK_TIMEOUT_SECS:-30}"

if [[ -z "$CONTRACT_ID" ]]; then
  printf 'Usage: CONTRACT_ID=<contract-id> [STELLAR_NETWORK=local] %s\n' "$SCRIPT_NAME" >&2
  exit 2
fi

if ! [[ "$CHECK_TIMEOUT_SECS" =~ ^[1-9][0-9]*$ ]]; then
  printf 'CHECK_TIMEOUT_SECS must be a positive integer\n' >&2
  exit 2
fi

if ! command -v stellar >/dev/null 2>&1; then
  printf 'stellar CLI is required\n' >&2
  exit 2
fi

temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/karis-preflight.XXXXXX")" || exit 1
declare -a check_names=()
declare -a check_pids=()
declare -a check_outputs=()

cleanup() {
  local pid
  for pid in "${check_pids[@]:-}"; do
    kill "$pid" 2>/dev/null || true
  done
  rm -rf "$temp_dir"
}
trap cleanup EXIT INT TERM

run_check() {
  local name="$1"
  local method="$2"
  local output_file="$temp_dir/$name"

  timeout --signal=TERM "${CHECK_TIMEOUT_SECS}s" \
    stellar contract invoke \
      --id "$CONTRACT_ID" \
      --network "$NETWORK" \
      -- "$method" >"$output_file" 2>&1
}

start_check() {
  local name="$1"
  local method="$2"

  check_names+=("$name")
  check_outputs+=("$temp_dir/$name")
  run_check "$name" "$method" &
  check_pids+=("$!")
}

# These read-only calls do not depend on one another, so start them together.
start_check version get_version
start_check escrow get_escrow
start_check legal_hold get_legal_hold
start_check funding_deadline get_funding_deadline

failed=0
for index in "${!check_pids[@]}"; do
  if wait "${check_pids[$index]}"; then
    printf '[PASS] %s\n' "${check_names[$index]}"
  else
    status=$?
    failed=1
    printf '[FAIL] %s (exit %d)\n' "${check_names[$index]}" "$status"
    sed 's/^/       /' "${check_outputs[$index]}"
  fi
done

if (( failed != 0 )); then
  printf 'Pre-flight checks failed\n' >&2
  exit 1
fi

printf 'Pre-flight checks passed\n'
