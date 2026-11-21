#!/usr/bin/env python3
"""
Validate that SCHEMA_VERSION in escrow/src/lib.rs matches the latest entry
in docs/VERSION_INTEROPERABILITY_MATRIX.md.

This script is run in CI to catch any drift between the source code schema
version and the documented compatibility matrix. A mismatch indicates that
the matrix was not updated when the schema version was bumped, which could
lead operators to attempt unsupported upgrade paths.

Exit codes:
  0 = Schema version matches matrix
  1 = Schema version does not match matrix (validation failure)
  2 = File not found or parsing error
"""

import re
import sys
from pathlib import Path


def extract_schema_version(lib_rs_path: Path) -> int:
    """
    Extract SCHEMA_VERSION from escrow/src/lib.rs.

    Expected format:
        pub const SCHEMA_VERSION: u32 = N;

    Args:
        lib_rs_path: Path to escrow/src/lib.rs

    Returns:
        The integer schema version value

    Raises:
        FileNotFoundError: If lib.rs is not found
        ValueError: If SCHEMA_VERSION constant is not found or not parseable
    """
    if not lib_rs_path.exists():
        raise FileNotFoundError(f"File not found: {lib_rs_path}")

    content = lib_rs_path.read_text(encoding="utf-8")

    # Match: pub const SCHEMA_VERSION: u32 = N;
    pattern = r"pub\s+const\s+SCHEMA_VERSION\s*:\s*u32\s*=\s*(\d+)\s*;"
    match = re.search(pattern, content)

    if not match:
        raise ValueError(
            "Could not find 'pub const SCHEMA_VERSION: u32 = N;' in lib.rs"
        )

    return int(match.group(1))


def extract_latest_version_from_matrix(matrix_path: Path) -> int:
    """
    Extract the latest schema version from VERSION_INTEROPERABILITY_MATRIX.md.

    Scans the "Quick Reference" table for the row marked with "**Current**"
    and extracts the version number from the first column.

    Expected format (in markdown table):
        | Schema Version | Status | ... |
        | 9 | **Current** | ... |

    Args:
        matrix_path: Path to docs/VERSION_INTEROPERABILITY_MATRIX.md

    Returns:
        The latest schema version from the matrix

    Raises:
        FileNotFoundError: If matrix is not found
        ValueError: If no **Current** marker is found
    """
    if not matrix_path.exists():
        raise FileNotFoundError(f"File not found: {matrix_path}")

    content = matrix_path.read_text(encoding="utf-8")

    # Look for rows in the Quick Reference table that have **Current**
    # Expected format: | 9 | **Current** | ... |
    pattern = r"\|\s*(\d+)\s*\|\s*\*\*Current\*\*\s*\|"
    match = re.search(pattern, content)

    if not match:
        raise ValueError(
            "Could not find a row with '**Current**' marker in the version matrix. "
            "The matrix must have a row like: | 9 | **Current** | ... |"
        )

    return int(match.group(1))


def extract_expected_schema_version_from_env(env_path: Path) -> int | None:
    """
    Extract EXPECTED_SCHEMA_VERSION from a .env file.

    Expected format:
        EXPECTED_SCHEMA_VERSION=8

    Args:
        env_path: Path to a .env file

    Returns:
        The version number if found, None if not present (optional in some .env files)

    Raises:
        FileNotFoundError: If env file is not found
        ValueError: If EXPECTED_SCHEMA_VERSION is present but not parseable
    """
    if not env_path.exists():
        raise FileNotFoundError(f"File not found: {env_path}")

    content = env_path.read_text(encoding="utf-8")

    # Match: EXPECTED_SCHEMA_VERSION=N (allowing for optional comments)
    pattern = r"^\s*EXPECTED_SCHEMA_VERSION\s*=\s*(\d+)\s*(?:#.*)?$"
    match = re.search(pattern, content, re.MULTILINE)

    if not match:
        # Field is optional in some .env files (not all have it)
        return None

    return int(match.group(1))


def validate_env_schema_versions(
    lib_rs_path: Path, env_file_dir: Path = None
) -> tuple[bool, str]:
    """
    Validate that all .env* files have EXPECTED_SCHEMA_VERSION matching SCHEMA_VERSION.

    Scans the repository root (or specified directory) for .env* files and checks
    that any EXPECTED_SCHEMA_VERSION values match the SCHEMA_VERSION from lib.rs.
    Missing EXPECTED_SCHEMA_VERSION fields are allowed (not all .env files have them).

    Args:
        lib_rs_path: Path to escrow/src/lib.rs
        env_file_dir: Directory to scan for .env files (default: current directory)

    Returns:
        A tuple (is_valid, message) where is_valid is True if all found versions match
    """
    if env_file_dir is None:
        env_file_dir = Path(".")

    try:
        code_version = extract_schema_version(lib_rs_path)
    except (FileNotFoundError, ValueError) as e:
        return False, f"✗ Failed to extract SCHEMA_VERSION: {e}"

    # Find all .env* files in the directory (non-recursive, top level only)
    env_files = sorted(env_file_dir.glob(".env*"))

    if not env_files:
        return (
            True,
            f"ℹ No .env* files found in {env_file_dir} (this is OK if using external config)",
        )

    mismatches = []
    checked_count = 0

    for env_file in env_files:
        try:
            expected_version = extract_expected_schema_version_from_env(env_file)
            if expected_version is None:
                # Field is optional; skip files that don't have it
                continue

            checked_count += 1
            if expected_version != code_version:
                mismatches.append(
                    {
                        "file": env_file.name,
                        "expected": expected_version,
                        "actual": code_version,
                    }
                )
        except (FileNotFoundError, ValueError) as e:
            return (
                False,
                f"✗ Error reading {env_file.name}: {e}",
            )

    if mismatches:
        msg = "✗ Schema version mismatch in .env files:\n"
        for m in mismatches:
            msg += (
                f"  {m['file']}: EXPECTED_SCHEMA_VERSION={m['expected']} "
                f"but SCHEMA_VERSION in lib.rs={m['actual']}\n"
            )
        msg += (
            f"\nAction: Update EXPECTED_SCHEMA_VERSION in .env* files to match "
            f"SCHEMA_VERSION={code_version} in escrow/src/lib.rs"
        )
        return False, msg

    if checked_count == 0:
        return (
            True,
            f"ℹ No EXPECTED_SCHEMA_VERSION fields found in .env* files (optional check)",
        )

    return (
        True,
        f"✓ Schema version .env validation passed: {checked_count} file(s) have "
        f"EXPECTED_SCHEMA_VERSION={code_version} matching lib.rs",
    )


def validate_schema_version(
    lib_rs_path: Path, matrix_path: Path
) -> tuple[bool, str]:
    """
    Validate that SCHEMA_VERSION matches the latest version in the matrix.

    Args:
        lib_rs_path: Path to escrow/src/lib.rs
        matrix_path: Path to docs/VERSION_INTEROPERABILITY_MATRIX.md

    Returns:
        A tuple (is_valid, message) where is_valid is True if versions match,
        and message is a descriptive string.
    """
    try:
        code_version = extract_schema_version(lib_rs_path)
        matrix_version = extract_latest_version_from_matrix(matrix_path)

        if code_version == matrix_version:
            msg = (
                f"✓ Schema version validation passed: "
                f"SCHEMA_VERSION={code_version} matches matrix version {matrix_version}"
            )
            return True, msg
        else:
            msg = (
                f"✗ Schema version mismatch detected:\n"
                f"  SCHEMA_VERSION in lib.rs: {code_version}\n"
                f"  Latest version in matrix: {matrix_version}\n"
                f"\n  Action: Update docs/VERSION_INTEROPERABILITY_MATRIX.md to reflect "
                f"schema version {code_version} as **Current**"
            )
            return False, msg

    except FileNotFoundError as e:
        return False, f"✗ File error: {e}"
    except ValueError as e:
        return False, f"✗ Parse error: {e}"


def main() -> int:
    """
    Main entry point for the validation script.

    Runs from the repository root; paths are relative to that.

    Performs two checks:
    1. SCHEMA_VERSION in escrow/src/lib.rs matches the matrix (version interoperability)
    2. EXPECTED_SCHEMA_VERSION in .env* files match the code version (deployment config)

    Returns:
        Exit code (0 for success, 1 for validation failure, 2 for file/parse error)
    """
    # Assume we run from the repository root
    lib_rs_path = Path("escrow/src/lib.rs")
    matrix_path = Path("docs/VERSION_INTEROPERABILITY_MATRIX.md")

    # Check 1: Validate against compatibility matrix
    is_valid_matrix, matrix_message = validate_schema_version(lib_rs_path, matrix_path)
    print(matrix_message)

    # Check 2: Validate .env* files
    is_valid_env, env_message = validate_env_schema_versions(lib_rs_path, Path("."))
    print(env_message)

    # Both checks must pass
    if is_valid_matrix and is_valid_env:
        return 0
    else:
        # Exit code 1 = validation failure (schema version mismatch)
        return 1


if __name__ == "__main__":
    sys.exit(main())
