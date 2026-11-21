#!/usr/bin/env python3
"""Check that ignored advisories in `deny.toml` and `.cargo/audit.toml` agree.

Issue 119: CI runs both `cargo audit` and `cargo deny`. If an advisory is
ignored in one tool but not the other, the two dependency-safety tools drift
apart. This script parses the ignored advisory IDs from both configuration
files, compares the two sets, prints any divergence, and exits non-zero when
an advisory is ignored in one but not the other.

Policy
------
Any advisory that is ignored must be ignored in *both* `deny.toml`
(`[advisories].ignore`) and `.cargo/audit.toml` (`[advisories].ignore`).
When adding or removing an ignore, update both files in the same change.

Usage
-----
    python3 scripts/check_advisory_ignore_sync.py [--deny PATH] [--audit PATH]

Exit codes:
    0 - the ignore lists agree (or both are missing/empty)
    1 - divergence detected
    2 - a configuration file could not be parsed
"""

from __future__ import annotations

import argparse
import os
import sys
from typing import Iterable, List, Optional, Set

try:  # Python 3.11+
    import tomllib  # type: ignore[import-not-found]
except ModuleNotFoundError:  # pragma: no cover - fallback for older Pythons
    try:
        import tomli as tomllib  # type: ignore[import-not-found,no-redef]
    except ModuleNotFoundError:  # pragma: no cover
        tomllib = None  # type: ignore[assignment]


DEFAULT_DENY_PATH = "deny.toml"
DEFAULT_AUDIT_PATH = os.path.join(".cargo", "audit.toml")


def _normalize_id(value: object) -> Optional[str]:
    """Return a normalized advisory ID string, or None if it is not usable."""
    if isinstance(value, str):
        stripped = value.strip()
        return stripped or None
    if isinstance(value, (int, float)):
        return str(value)
    return None


def _extract_ignore_ids(data: object) -> Set[str]:
    """Extract advisory IDs from a parsed TOML document.

    Supports the common shapes used by `cargo deny` and `cargo audit`:

        [advisories]
        ignore = ["RUSTSEC-2021-0001", ...]

    and the table form:

        [advisories]
        ignore = [
            { id = "RUSTSEC-2021-0001", reason = "..." },
        ]

    Missing or empty ignore lists yield an empty set.
    """
    ids: Set[str] = set()
    if not isinstance(data, dict):
        return ids

    advisories = data.get("advisories")
    if not isinstance(advisories, dict):
        return ids

    ignore = advisories.get("ignore")
    if ignore is None:
        return ids

    if isinstance(ignore, (list, tuple)):
        for entry in ignore:
            if isinstance(entry, dict):
                # Table form: { id = "...", reason = "..." }
                for key in ("id", "advisory", "advisory-id"):
                    if key in entry:
                        normalized = _normalize_id(entry[key])
                        if normalized:
                            ids.add(normalized)
                        break
            else:
                normalized = _normalize_id(entry)
                if normalized:
                    ids.add(normalized)
    else:
        # A single scalar value is unusual but tolerated.
        normalized = _normalize_id(ignore)
        if normalized:
            ids.add(normalized)

    return ids


def _load_ignore_ids(path: str, label: str) -> Set[str]:
    """Load ignored advisory IDs from a TOML file.

    A missing file is treated as an empty ignore list.
    """
    if not os.path.exists(path):
        print(f"note: {label} config not found at {path!r}; treating as empty")
        return set()

    if tomllib is None:
        print(
            "error: no TOML parser available (need Python 3.11+ or the "
            "'tomli' package)",
            file=sys.stderr,
        )
        raise SystemExit(2)

    try:
        with open(path, "rb") as handle:
            data = tomllib.load(handle)
    except (OSError, ValueError) as exc:
        print(f"error: failed to parse {label} config {path!r}: {exc}", file=sys.stderr)
        raise SystemExit(2)

    return _extract_ignore_ids(data)


def _format_ids(ids: Iterable[str]) -> str:
    ordered: List[str] = sorted(ids)
    if not ordered:
        return "  (none)"
    return "\n".join(f"  - {advisory_id}" for advisory_id in ordered)


def check_sync(deny_ids: Set[str], audit_ids: Set[str]) -> bool:
    """Print divergence between the two sets. Return True when in sync."""
    only_deny = deny_ids - audit_ids
    only_audit = audit_ids - deny_ids

    if not only_deny and not only_audit:
        print(
            "OK: ignored advisories agree between deny.toml and "
            f".cargo/audit.toml ({len(deny_ids)} ignored)"
        )
        return True

    print("ERROR: ignored advisories diverge between the two tools.")
    if only_deny:
        print("\nIgnored in deny.toml but NOT in .cargo/audit.toml:")
        print(_format_ids(only_deny))
    if only_audit:
        print("\nIgnored in .cargo/audit.toml but NOT in deny.toml:")
        print(_format_ids(only_audit))
    print(
        "\nPolicy: an advisory must be ignored in BOTH deny.toml "
        "([advisories].ignore) and .cargo/audit.toml ([advisories].ignore). "
        "Update both files in the same change."
    )
    return False


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Verify that ignored advisories in deny.toml and "
            ".cargo/audit.toml agree."
        )
    )
    parser.add_argument(
        "--deny",
        default=DEFAULT_DENY_PATH,
        help=f"path to deny.toml (default: {DEFAULT_DENY_PATH})",
    )
    parser.add_argument(
        "--audit",
        default=DEFAULT_AUDIT_PATH,
        help=f"path to audit.toml (default: {DEFAULT_AUDIT_PATH})",
    )
    args = parser.parse_args(argv)

    deny_ids = _load_ignore_ids(args.deny, "deny.toml")
    audit_ids = _load_ignore_ids(args.audit, "audit.toml")

    return 0 if check_sync(deny_ids, audit_ids) else 1


if __name__ == "__main__":
    sys.exit(main())