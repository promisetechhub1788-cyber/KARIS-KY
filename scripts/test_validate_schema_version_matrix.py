#!/usr/bin/env python3
"""
Tests for validate_schema_version_matrix.py

Tests the mismatch detection case and parsing logic.
Can be run standalone or via pytest.
"""

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

# Import the module under test
from validate_schema_version_matrix import (
    extract_schema_version,
    extract_latest_version_from_matrix,
    extract_expected_schema_version_from_env,
    validate_schema_version,
    validate_env_schema_versions,
)

# Repository root (this test file lives in scripts/)
REPO_ROOT = Path(__file__).resolve().parent.parent
PREFLIGHT_SCRIPT = REPO_ROOT / "scripts" / "release_preflight.sh"
=======


def test_extract_schema_version_valid():
    """Test extracting a valid schema version from lib.rs content."""
    with tempfile.TemporaryDirectory() as tmpdir:
        lib_rs = Path(tmpdir) / "lib.rs"
        lib_rs.write_text(
            """
pub const SCHEMA_VERSION: u32 = 9;
        """
        )
        version = extract_schema_version(lib_rs)
        assert version == 9, f"Expected 9, got {version}"
        print("✓ test_extract_schema_version_valid passed")


def test_extract_schema_version_with_whitespace():
    """Test extracting version with various whitespace."""
    with tempfile.TemporaryDirectory() as tmpdir:
        lib_rs = Path(tmpdir) / "lib.rs"
        lib_rs.write_text(
            """
pub   const   SCHEMA_VERSION   :   u32   =   7   ;
        """
        )
        version = extract_schema_version(lib_rs)
        assert version == 7, f"Expected 7, got {version}"
        print("✓ test_extract_schema_version_with_whitespace passed")


def test_extract_schema_version_not_found():
    """Test error handling when SCHEMA_VERSION is missing."""
    with tempfile.TemporaryDirectory() as tmpdir:
        lib_rs = Path(tmpdir) / "lib.rs"
        lib_rs.write_text("// No schema version here")
        try:
            extract_schema_version(lib_rs)
            assert False, "Should have raised ValueError"
        except ValueError as e:
            assert "Could not find" in str(e)
            print("✓ test_extract_schema_version_not_found passed")


def test_extract_schema_version_file_not_found():
    """Test error handling when lib.rs file doesn't exist."""
    try:
        extract_schema_version(Path("/nonexistent/lib.rs"))
        assert False, "Should have raised FileNotFoundError"
    except FileNotFoundError:
        print("✓ test_extract_schema_version_file_not_found passed")


def test_extract_latest_version_from_matrix_valid():
    """Test extracting the latest version from matrix markdown."""
    with tempfile.TemporaryDirectory() as tmpdir:
        matrix = Path(tmpdir) / "matrix.md"
        matrix.write_text(
            """
# Schema Version Matrix

| Schema Version | Status | ... |
| 1 | Deprecated | ... |
| 9 | **Current** | ... |
        """
        )
        version = extract_latest_version_from_matrix(matrix)
        assert version == 9, f"Expected 9, got {version}"
        print("✓ test_extract_latest_version_from_matrix_valid passed")


def test_extract_latest_version_from_matrix_whitespace():
    """Test extracting version with variable whitespace in markdown."""
    with tempfile.TemporaryDirectory() as tmpdir:
        matrix = Path(tmpdir) / "matrix.md"
        matrix.write_text(
            """
| 6 |  **Current**  |  Features  |
        """
        )
        version = extract_latest_version_from_matrix(matrix)
        assert version == 6, f"Expected 6, got {version}"
        print("✓ test_extract_latest_version_from_matrix_whitespace passed")


def test_extract_latest_version_from_matrix_not_found():
    """Test error handling when **Current** marker is missing."""
    with tempfile.TemporaryDirectory() as tmpdir:
        matrix = Path(tmpdir) / "matrix.md"
        matrix.write_text(
            """
| Schema Version | Status |
| 1 | Deprecated |
| 9 | Future |
        """
        )
        try:
            extract_latest_version_from_matrix(matrix)
            assert False, "Should have raised ValueError"
        except ValueError as e:
            assert "**Current**" in str(e)
            print("✓ test_extract_latest_version_from_matrix_not_found passed")


def test_validate_schema_version_match():
    """Test validation when versions match."""
    with tempfile.TemporaryDirectory() as tmpdir:
        lib_rs = Path(tmpdir) / "lib.rs"
        matrix = Path(tmpdir) / "matrix.md"

        lib_rs.write_text("pub const SCHEMA_VERSION: u32 = 9;")
        matrix.write_text("| 9 | **Current** | Features |")

        is_valid, message = validate_schema_version(lib_rs, matrix)
        assert is_valid, f"Validation should succeed: {message}"
        assert "✓" in message
        print("✓ test_validate_schema_version_match passed")


def test_validate_schema_version_mismatch():
    """Test validation failure when versions don't match (the key test case)."""
    with tempfile.TemporaryDirectory() as tmpdir:
        lib_rs = Path(tmpdir) / "lib.rs"
        matrix = Path(tmpdir) / "matrix.md"

        # lib.rs says version 10, but matrix says version 9 is current
        lib_rs.write_text("pub const SCHEMA_VERSION: u32 = 10;")
        matrix.write_text("| 9 | **Current** | Features |")

        is_valid, message = validate_schema_version(lib_rs, matrix)
        assert not is_valid, f"Validation should fail: {message}"
        assert "✗" in message
        assert "mismatch" in message.lower()
        assert "10" in message
        assert "9" in message
        print("✓ test_validate_schema_version_mismatch passed (THIS IS THE KEY TEST)")


def test_validate_schema_version_multiple_entries():
    """Test when matrix has multiple version entries."""
    with tempfile.TemporaryDirectory() as tmpdir:
        lib_rs = Path(tmpdir) / "lib.rs"
        matrix = Path(tmpdir) / "matrix.md"

        lib_rs.write_text("pub const SCHEMA_VERSION: u32 = 8;")
        matrix.write_text(
            """
# Schema Version Matrix

| Schema Version | Status |
| 6 | Deprecated |
| 7 | Deprecated |
| 8 | **Current** |
| 9 | Planned |
        """
        )

        is_valid, message = validate_schema_version(lib_rs, matrix)
        assert is_valid, f"Validation should succeed: {message}"
        print("✓ test_validate_schema_version_multiple_entries passed")


def test_extract_expected_schema_version_from_env_valid():
    """Test extracting EXPECTED_SCHEMA_VERSION from .env file."""
    with tempfile.TemporaryDirectory() as tmpdir:
        env_file = Path(tmpdir) / ".env.testnet"
        env_file.write_text(
            """
# Test env file
STELLAR_NETWORK=testnet
EXPECTED_SCHEMA_VERSION=8
SOROBAN_RPC_URL=https://example.com
        """
        )
        version = extract_expected_schema_version_from_env(env_file)
        assert version == 8, f"Expected 8, got {version}"
        print("✓ test_extract_expected_schema_version_from_env_valid passed")


def test_extract_expected_schema_version_from_env_with_comment():
    """Test extracting version with trailing comment."""
    with tempfile.TemporaryDirectory() as tmpdir:
        env_file = Path(tmpdir) / ".env"
        env_file.write_text("EXPECTED_SCHEMA_VERSION=7 # Schema version comment")
        version = extract_expected_schema_version_from_env(env_file)
        assert version == 7, f"Expected 7, got {version}"
        print("✓ test_extract_expected_schema_version_from_env_with_comment passed")


def test_extract_expected_schema_version_from_env_not_present():
    """Test that missing EXPECTED_SCHEMA_VERSION returns None."""
    with tempfile.TemporaryDirectory() as tmpdir:
        env_file = Path(tmpdir) / ".env"
        env_file.write_text("STELLAR_NETWORK=testnet\nSOROBAN_RPC_URL=https://example.com")
        version = extract_expected_schema_version_from_env(env_file)
        assert version is None, f"Expected None for missing field, got {version}"
        print("✓ test_extract_expected_schema_version_from_env_not_present passed")


def test_extract_expected_schema_version_from_env_not_found():
    """Test error when .env file doesn't exist."""
    try:
        extract_expected_schema_version_from_env(Path("/nonexistent/.env"))
        assert False, "Should have raised FileNotFoundError"
    except FileNotFoundError:
        print("✓ test_extract_expected_schema_version_from_env_not_found passed")


def test_validate_env_schema_versions_match():
    """Test successful validation when .env EXPECTED_SCHEMA_VERSION matches code."""
    with tempfile.TemporaryDirectory() as tmpdir:
        tmpdir_path = Path(tmpdir)
        lib_rs = tmpdir_path / "lib.rs"
        env_file = tmpdir_path / ".env.testnet"

        lib_rs.write_text("pub const SCHEMA_VERSION: u32 = 8;")
        env_file.write_text("EXPECTED_SCHEMA_VERSION=8\nSTELLAR_NETWORK=testnet")

        is_valid, message = validate_env_schema_versions(lib_rs, tmpdir_path)
        assert is_valid, f"Validation should succeed: {message}"
        assert "✓" in message or "ℹ" in message
        print("✓ test_validate_env_schema_versions_match passed")


def test_validate_env_schema_versions_mismatch():
    """Test validation failure when .env EXPECTED_SCHEMA_VERSION doesn't match."""
    with tempfile.TemporaryDirectory() as tmpdir:
        tmpdir_path = Path(tmpdir)
        lib_rs = tmpdir_path / "lib.rs"
        env_file = tmpdir_path / ".env.testnet"

        lib_rs.write_text("pub const SCHEMA_VERSION: u32 = 8;")
        env_file.write_text("EXPECTED_SCHEMA_VERSION=7\nSTELLAR_NETWORK=testnet")

        is_valid, message = validate_env_schema_versions(lib_rs, tmpdir_path)
        assert not is_valid, f"Validation should fail: {message}"
        assert "✗" in message
        assert "mismatch" in message.lower()
        assert "8" in message
        assert "7" in message
        print("✓ test_validate_env_schema_versions_mismatch passed (KEY .ENV TEST)")


def test_validate_env_schema_versions_multiple_files():
    """Test validation with multiple .env files."""
    with tempfile.TemporaryDirectory() as tmpdir:
        tmpdir_path = Path(tmpdir)
        lib_rs = tmpdir_path / "lib.rs"
        env1 = tmpdir_path / ".env"
        env2 = tmpdir_path / ".env.testnet"
        env3 = tmpdir_path / ".env.prod"

        lib_rs.write_text("pub const SCHEMA_VERSION: u32 = 8;")
        env1.write_text("EXPECTED_SCHEMA_VERSION=8")
        env2.write_text("EXPECTED_SCHEMA_VERSION=8\nSTELLAR_NETWORK=testnet")
        env3.write_text("EXPECTED_SCHEMA_VERSION=8\nSTELLAR_NETWORK=public")

        is_valid, message = validate_env_schema_versions(lib_rs, tmpdir_path)
        assert is_valid, f"All files should match: {message}"
        assert "3" in message  # Should report 3 files checked
        print("✓ test_validate_env_schema_versions_multiple_files passed")


def test_validate_env_schema_versions_mixed_present_absent():
    """Test validation when only some .env files have EXPECTED_SCHEMA_VERSION."""
    with tempfile.TemporaryDirectory() as tmpdir:
        tmpdir_path = Path(tmpdir)
        lib_rs = tmpdir_path / "lib.rs"
        env1 = tmpdir_path / ".env.testnet"
        env2 = tmpdir_path / ".env.local"

        lib_rs.write_text("pub const SCHEMA_VERSION: u32 = 8;")
        env1.write_text("EXPECTED_SCHEMA_VERSION=8")
        env2.write_text("# Local config - no schema version")

        is_valid, message = validate_env_schema_versions(lib_rs, tmpdir_path)
        assert is_valid, f"Should pass when one has matching version: {message}"
        assert "1" in message  # Should report 1 file checked
        print("✓ test_validate_env_schema_versions_mixed_present_absent passed")


def test_validate_env_schema_versions_no_files():
    """Test validation when no .env files exist."""
    with tempfile.TemporaryDirectory() as tmpdir:
        tmpdir_path = Path(tmpdir)
        lib_rs = tmpdir_path / "lib.rs"

        lib_rs.write_text("pub const SCHEMA_VERSION: u32 = 8;")

        is_valid, message = validate_env_schema_versions(lib_rs, tmpdir_path)
        assert is_valid, f"Should pass when no .env files: {message}"
        assert "No .env* files" in message or "optional" in message.lower()
        print("✓ test_validate_env_schema_versions_no_files passed")


def _run_preflight(repo_root):
    """Run the release preflight script against the given repo root."""
    return subprocess.run(
        ["bash", str(PREFLIGHT_SCRIPT)],
        cwd=str(repo_root),
        capture_output=True,
        text=True,
    )


def test_release_preflight_clean_repo_passes():
    """Test that the preflight script exits 0 on the clean repository."""
    assert PREFLIGHT_SCRIPT.exists(), f"Missing preflight script: {PREFLIGHT_SCRIPT}"
    result = _run_preflight(REPO_ROOT)
    assert result.returncode == 0, (
        "Preflight should pass on clean repo but exited "
        f"{result.returncode}\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    )
    print("✓ test_release_preflight_clean_repo_passes passed")


def test_release_preflight_drifted_repo_fails():
    """Test that the preflight script exits non-zero on a drifted repo copy."""
    assert PREFLIGHT_SCRIPT.exists(), f"Missing preflight script: {PREFLIGHT_SCRIPT}"

    with tempfile.TemporaryDirectory() as tmpdir:
        repo_copy = Path(tmpdir) / "repo"
        shutil.copytree(
            REPO_ROOT,
            repo_copy,
            ignore=shutil.ignore_patterns(".git", "target", "__pycache__", "node_modules"),
        )

        # Introduce drift: bump SCHEMA_VERSION in lib.rs so it no longer
        # matches the schema version matrix / spec metadata.
        lib_rs = repo_copy / "contracts" / "schema" / "src" / "lib.rs"
        assert lib_rs.exists(), f"Expected lib.rs at {lib_rs}"
        content = lib_rs.read_text()
        import re

        drifted, count = re.subn(
            r"(pub\s+const\s+SCHEMA_VERSION\s*:\s*u32\s*=\s*)(\d+)",
            lambda m: f"{m.group(1)}{int(m.group(2)) + 1}",
            content,
        )
        assert count == 1, "Expected exactly one SCHEMA_VERSION declaration"
        lib_rs.write_text(drifted)

        result = _run_preflight(repo_copy)
        assert result.returncode != 0, (
            "Preflight should fail on drifted repo but exited 0\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
        print("✓ test_release_preflight_drifted_repo_fails passed")


def run_all_tests():
    """Run all tests and report results."""
    tests = [
=======
        test_extract_schema_version_valid,
        test_extract_schema_version_with_whitespace,
        test_extract_schema_version_not_found,
        test_extract_schema_version_file_not_found,
        test_extract_latest_version_from_matrix_valid,
        test_extract_latest_version_from_matrix_whitespace,
        test_extract_latest_version_from_matrix_not_found,
        test_validate_schema_version_match,
        test_validate_schema_version_mismatch,
        test_validate_schema_version_multiple_entries,
        # New .env validation tests
        test_extract_expected_schema_version_from_env_valid,
        test_extract_expected_schema_version_from_env_with_comment,
        test_extract_expected_schema_version_from_env_not_present,
        test_extract_expected_schema_version_from_env_not_found,
        test_validate_env_schema_versions_match,
        test_validate_env_schema_versions_mismatch,
        test_validate_env_schema_versions_multiple_files,
        test_validate_env_schema_versions_mixed_present_absent,
        test_validate_env_schema_versions_no_files,
        # Release preflight tests
        test_release_preflight_clean_repo_passes,
        test_release_preflight_drifted_repo_fails,
    ]
=======

    passed = 0
    failed = 0

    for test in tests:
        try:
            test()
            passed += 1
        except Exception as e:
            print(f"✗ {test.__name__} failed: {e}")
            failed += 1

    print(f"\n{passed} passed, {failed} failed")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(run_all_tests())
