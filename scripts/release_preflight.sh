#!/usr/bin/env bash
#
# release_preflight.sh — Single go/no-go gate for release readiness.
#
# Runs three validators in sequence:
#   1. Build metadata validation   (scripts/validate_build_metadata.py)
#   2. Spec contract validation    (sdk-ts/spec.json)
#   3. Schema version matrix       (scripts/validate_schema_version_matrix.py)
#
# Prints a per-check PASS/FAIL summary and exits non-zero if any check fails.
#
# Usage:
#   scripts/release_preflight.sh
#
# Exit codes:
#   0  All checks passed (GO)
#   1  One or more checks failed (NO-GO)
#
set -euo pipefail

# Resolve the repository root regardless of where the script is invoked from.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

# ---------------------------------------------------------------------------
# Check definitions
# ---------------------------------------------------------------------------
CHECK_NAMES=()
CHECK_RESULTS=()
CHECK_OUTPUTS=()

# run_check <name> <command...>
# Runs a command, capturing output and exit status without aborting the script.
run_check() {
  local name="$1"
  shift
  local output
  local status=0

  if output="$("$@" 2>&1)"; then
    status=0
  else
    status=$?
  fi

  CHECK_NAMES+=("${name}")
  if [[ ${status} -eq 0 ]]; then
    CHECK_RESULTS+=("PASS")
  else
    CHECK_RESULTS+=("FAIL")
  fi
  CHECK_OUTPUTS+=("${output}")
  return 0
}

# ---------------------------------------------------------------------------
# Check 1: Build metadata
# ---------------------------------------------------------------------------
run_check "build-metadata" python3 scripts/validate_build_metadata.py

# ---------------------------------------------------------------------------
# Check 2: Spec contract (sdk-ts/spec.json)
# ---------------------------------------------------------------------------
validate_spec() {
  local spec_path="sdk-ts/spec.json"

  if [[ ! -f "${spec_path}" ]]; then
    echo "ERROR: spec file not found: ${spec_path}" >&2
    return 1
  fi

  python3 - "${spec_path}" <<'PY'
import json
import sys

spec_path = sys.argv[1]

try:
    with open(spec_path, "r", encoding="utf-8") as fh:
        spec = json.load(fh)
except json.JSONDecodeError as exc:
    print(f"ERROR: {spec_path} is not valid JSON: {exc}", file=sys.stderr)
    sys.exit(1)

if not isinstance(spec, dict):
    print(f"ERROR: {spec_path} must contain a JSON object", file=sys.stderr)
    sys.exit(1)

errors = []

# Required top-level contract fields.
required_fields = ["name", "version"]
for field in required_fields:
    if field not in spec:
        errors.append(f"missing required field: {field!r}")
    elif not isinstance(spec[field], str) or not spec[field].strip():
        errors.append(f"field {field!r} must be a non-empty string")

# Version must look like a semantic version (e.g. 1.2.3, optionally with
# a pre-release/build suffix).
import re

version = spec.get("version")
if isinstance(version, str) and version.strip():
    semver_re = re.compile(
        r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$"
    )
    if not semver_re.match(version.strip()):
        errors.append(f"version {version!r} is not a valid semantic version")

if errors:
    for err in errors:
        print(f"ERROR: {err}", file=sys.stderr)
    sys.exit(1)

print(f"OK: {spec_path} validated (name={spec['name']!r}, version={spec['version']!r})")
PY
}

run_check "spec-contract" validate_spec

# ---------------------------------------------------------------------------
# Check 3: Schema version matrix
# ---------------------------------------------------------------------------
run_check "schema-matrix" python3 scripts/validate_schema_version_matrix.py

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "=========================================="
echo " Release Preflight Summary"
echo "=========================================="

overall_status=0
for i in "${!CHECK_NAMES[@]}"; do
  name="${CHECK_NAMES[$i]}"
  result="${CHECK_RESULTS[$i]}"
  printf ' [%s] %s\n' "${result}" "${name}"
  if [[ "${result}" != "PASS" ]]; then
    overall_status=1
    # Surface the failing check's output to aid debugging.
    if [[ -n "${CHECK_OUTPUTS[$i]}" ]]; then
      while IFS= read -r line; do
        printf '        %s\n' "${line}"
      done <<< "${CHECK_OUTPUTS[$i]}"
    fi
  fi
done

echo "------------------------------------------"
if [[ ${overall_status} -eq 0 ]]; then
  echo " RESULT: GO — all checks passed"
  echo "=========================================="
  exit 0
else
  echo " RESULT: NO-GO — one or more checks failed"
  echo "=========================================="
  exit 1
fi