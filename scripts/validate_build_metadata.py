#!/usr/bin/env python3
"""
Validate that the WASM build artifact's embedded metadata matches the
source-of-truth constants in `escrow/src/lib.rs`.

Reads:
- `target/build_metadata.json` — JSON sidecar written by `escrow/build.rs` at
  compile time (includes `schema_version`, `interface_version`, `git_commit`,
  `build_timestamp`, `pkg_version`, `rust_version`).
- `escrow/src/lib.rs` — source of truth for `SCHEMA_VERSION` and
  `CONTRACT_INTERFACE_VERSION`.

Exits 0 on success, 1 on failure. Designed for CI gating.

Usage:
    python3 scripts/validate_build_metadata.py
    python3 scripts/validate_build_metadata.py \\
        --json target/build_metadata.json \\
        --lib escrow/src/lib.rs
    python3 scripts/validate_build_metadata.py \\
        --commit "$GITHUB_SHA" --schema-version 1
"""

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

DEFAULT_JSON = "target/build_metadata.json"
DEFAULT_LIB = "escrow/src/lib.rs"

SCHEMA_VERSION_RE = re.compile(
    r"pub\s+const\s+SCHEMA_VERSION\s*:\s*u32\s*=\s*(\d+)\s*;"
)
INTERFACE_VERSION_RE = re.compile(
    r"pub\s+const\s+CONTRACT_INTERFACE_VERSION\s*:\s*u32\s*=\s*(\d+)\s*;"
)


def load_metadata(json_path: Path) -> dict:
    if not json_path.exists():
        print(f"ERROR: {json_path} not found.", file=sys.stderr)
        print("       Did `cargo build` (or `cargo build --target wasm32v1-none`)", file=sys.stderr)
        print("       run successfully? build.rs writes this file at compile time.", file=sys.stderr)
        sys.exit(1)
    try:
        with json_path.open() as fh:
            return json.load(fh)
    except json.JSONDecodeError as exc:
        print(f"ERROR: failed to parse {json_path}: {exc}", file=sys.stderr)
        sys.exit(1)


def extract_source_constants(lib_rs: Path) -> tuple[int, int]:
    content = lib_rs.read_text()

    schema_match = SCHEMA_VERSION_RE.search(content)
    if not schema_match:
        print(f"ERROR: SCHEMA_VERSION constant not found in {lib_rs}", file=sys.stderr)
        sys.exit(1)

    interface_match = INTERFACE_VERSION_RE.search(content)
    if not interface_match:
        print(
            f"ERROR: CONTRACT_INTERFACE_VERSION constant not found in {lib_rs}",
            file=sys.stderr,
        )
        sys.exit(1)

    return int(schema_match.group(1)), int(interface_match.group(1))


def resolve_expected_commit(explicit: str | None) -> str | None:
    """Resolve the expected commit SHA from the flag or the CI environment."""
    if explicit:
        return explicit.strip()
    for env_var in ("GITHUB_SHA", "CI_COMMIT_SHA", "GIT_COMMIT"):
        value = os.environ.get(env_var)
        if value:
            return value.strip()
    try:
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
            check=True,
        )
        return result.stdout.strip()
    except (subprocess.CalledProcessError, FileNotFoundError):
        return None


def commits_match(artifact_commit: str, expected_commit: str) -> bool:
    """Compare commits allowing for short/full SHA representations."""
    artifact_commit = artifact_commit.strip().lower()
    expected_commit = expected_commit.strip().lower()
    if artifact_commit == expected_commit:
        return True
    # Allow prefix matching in either direction (short vs full SHA).
    return artifact_commit.startswith(expected_commit) or expected_commit.startswith(
        artifact_commit
    )


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Validate embedded build metadata against source constants."
    )
    parser.add_argument(
        "--json",
        default=DEFAULT_JSON,
        help=f"Path to build_metadata.json (default: {DEFAULT_JSON})",
    )
    parser.add_argument(
        "--lib",
        default=DEFAULT_LIB,
        help=f"Path to lib.rs (default: {DEFAULT_LIB})",
    )
    parser.add_argument(
        "--commit",
        default=None,
        help=(
            "Expected commit SHA. Defaults to $GITHUB_SHA / $CI_COMMIT_SHA / "
            "$GIT_COMMIT, then `git rev-parse HEAD`."
        ),
    )
    parser.add_argument(
        "--schema-version",
        type=int,
        default=None,
        help=(
            "Expected SCHEMA_VERSION. Defaults to the value parsed from --lib."
        ),
    )
    args = parser.parse_args()

    json_path = Path(args.json)
    lib_path = Path(args.lib)

    metadata = load_metadata(json_path)
    schema_src, interface_src = extract_source_constants(lib_path)

    expected_schema = (
        args.schema_version if args.schema_version is not None else schema_src
    )
    expected_commit = resolve_expected_commit(args.commit)

    artifact_schema = metadata.get("schema_version")
    artifact_interface = metadata.get("interface_version")
    git_commit = metadata.get("git_commit", "<missing>")
    build_ts = metadata.get("build_timestamp", "<missing>")
    pkg_version = metadata.get("pkg_version", "<missing>")
    rust_version = metadata.get("rust_version", "<missing>")

    print("─" * 60)
    print("Build metadata validation")
    print("─" * 60)
    print(f"  Source  SCHEMA_VERSION       : {schema_src}")
    print(f"  Expected SCHEMA_VERSION      : {expected_schema}")
    print(f"  Artifact SCHEMA_VERSION      : {artifact_schema}")
    print(f"  Source  INTERFACE_VERSION    : {interface_src}")
    print(f"  Artifact INTERFACE_VERSION   : {artifact_interface}")
    print(f"  Expected commit              : {expected_commit or '<unresolved>'}")
    print(f"  Git commit (short)           : {git_commit}")
    print(f"  Build timestamp (UTC)        : {build_ts}")
    print(f"  Cargo package version        : {pkg_version}")
    print(f"  Rust compiler version        : {rust_version}")
    print("─" * 60)

    failed = False

    if artifact_schema != expected_schema:
        print(
            f"FAIL  SCHEMA_VERSION mismatch (expected={expected_schema}, "
            f"artifact={artifact_schema})",
            file=sys.stderr,
        )
        failed = True
    else:
        print(f"OK    SCHEMA_VERSION matches ({expected_schema})")

    if artifact_interface != interface_src:
        print(
            f"FAIL  INTERFACE_VERSION mismatch (source={interface_src}, "
            f"artifact={artifact_interface})",
            file=sys.stderr,
        )
        failed = True
    else:
        print(f"OK    INTERFACE_VERSION matches ({interface_src})")

    if expected_commit:
        if not git_commit or git_commit == "unknown":
            print(
                f"FAIL  git_commit is '{git_commit}' but expected "
                f"{expected_commit}",
                file=sys.stderr,
            )
            failed = True
        elif not commits_match(str(git_commit), expected_commit):
            print(
                f"FAIL  git_commit mismatch (expected={expected_commit}, "
                f"artifact={git_commit})",
                file=sys.stderr,
            )
            failed = True
        else:
            print(f"OK    git_commit matches ({git_commit})")
    else:
        print(
            "WARN  expected commit could not be resolved; skipping commit check",
            file=sys.stderr,
        )

    if not build_ts or build_ts == "unknown":
        print(
            f"WARN  build_timestamp is '{build_ts}' — was the `date` command available?",
            file=sys.stderr,
        )

    print("─" * 60)
    if failed:
        print("RESULT: FAILED", file=sys.stderr)
        sys.exit(1)
    print("RESULT: PASSED")
    sys.exit(0)


if __name__ == "__main__":
    main()