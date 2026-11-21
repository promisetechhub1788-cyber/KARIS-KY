#!/usr/bin/env python3
"""
WASM size check script for karis-ky escrow contract.

Usage:
    python3 scripts/check_wasm_size.py [--wasm PATH] [--baseline PATH] [--record]

Exits with code 1 if:
  - The WASM file exceeds the absolute size limit (1 MB)
  - The WASM file has grown more than 10% compared to the baseline

With --record, updates the baseline JSON with the current size and metadata.
"""

import argparse
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
DEFAULT_WASM = "target/wasm32-unknown-unknown/release/karis_ky_escrow.wasm"
DEFAULT_BASELINE = "scripts/wasm_size_baseline.json"

ABSOLUTE_LIMIT_BYTES = 1_000_000          # 1 MB hard cap
RELATIVE_INCREASE_THRESHOLD = 0.10        # 10% growth hard-fail threshold

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _human(size_bytes: int) -> str:
    """Return a human-readable size string."""
    if size_bytes < 1024:
        return f"{size_bytes} B"
    kb = size_bytes / 1024
    if kb < 1024:
        return f"{kb:.2f} KB"
    return f"{kb / 1024:.2f} MB"


def load_baseline(path: Path) -> dict:
    """Load baseline JSON; return empty structure if file is missing."""
    if not path.exists():
        return {"baseline_bytes": None, "history": []}
    with path.open() as fh:
        return json.load(fh)


def save_baseline(path: Path, data: dict) -> None:
    """Persist baseline JSON."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w") as fh:
        json.dump(data, fh, indent=2)
        fh.write("\n")


def get_git_info() -> dict:
    """Return best-effort git metadata (non-fatal on failure)."""
    import subprocess  # noqa: PLC0415

    info: dict = {}
    try:
        info["commit"] = subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"], text=True
        ).strip()
    except Exception:  # noqa: BLE001
        info["commit"] = "unknown"

    try:
        info["ref"] = (
            os.environ.get("GITHUB_REF_NAME")
            or subprocess.check_output(
                ["git", "rev-parse", "--abbrev-ref", "HEAD"], text=True
            ).strip()
        )
    except Exception:  # noqa: BLE001
        info["ref"] = "unknown"

    return info


# ---------------------------------------------------------------------------
# Core logic
# ---------------------------------------------------------------------------

def check_size(wasm_path: Path, baseline_path: Path, record: bool) -> int:
    """
    Check WASM size against baseline and absolute limit.

    Returns an exit code (0 = OK, 1 = alert/error).
    """
    # --- Validate artifact exists ---
    if not wasm_path.exists():
        print(f"ERROR: WASM artifact not found: {wasm_path}", file=sys.stderr)
        return 1

    current_bytes = wasm_path.stat().st_size
    now = datetime.now(tz=timezone.utc).isoformat()
    git = get_git_info()

    print(f"WASM artifact : {wasm_path}")
    print(f"Current size  : {_human(current_bytes)} ({current_bytes:,} bytes)")
    print(f"Absolute limit: {_human(ABSOLUTE_LIMIT_BYTES)} ({ABSOLUTE_LIMIT_BYTES:,} bytes)")
    print()

    baseline_data = load_baseline(baseline_path)
    baseline_bytes: int | None = baseline_data.get("baseline_bytes")

    failed = False

    # --- Absolute size check ---
    if current_bytes > ABSOLUTE_LIMIT_BYTES:
        over = current_bytes - ABSOLUTE_LIMIT_BYTES
        print(
            f"ALERT  [absolute] Size {_human(current_bytes)} exceeds 1 MB limit "
            f"(over by {_human(over)}).",
            file=sys.stderr,
        )
        failed = True
    else:
        headroom = ABSOLUTE_LIMIT_BYTES - current_bytes
        print(f"OK     [absolute] {_human(headroom)} below 1 MB limit.")

    # --- Relative growth check ---
    if baseline_bytes is not None:
        print(f"Baseline size : {_human(baseline_bytes)} ({baseline_bytes:,} bytes)")
        delta = current_bytes - baseline_bytes
        pct = delta / baseline_bytes if baseline_bytes else 0.0
        direction = "+" if delta >= 0 else ""
        print(f"Delta         : {direction}{_human(abs(delta))} ({direction}{pct:.1%})")
        print()

        if pct > RELATIVE_INCREASE_THRESHOLD:
            # Hard CI failure: growth beyond the threshold is not merely reported,
            # it sets `failed` so check_size() returns a non-zero exit code.
            print(
                f"ALERT  [relative] Size grew {pct:.1%} (threshold: {RELATIVE_INCREASE_THRESHOLD:.0%}). "
                f"Baseline was {_human(baseline_bytes)}, now {_human(current_bytes)}.",
                file=sys.stderr,
            )
            failed = True
        elif delta > 0:
            print(
                f"OK     [relative] Size grew {pct:.1%} — within {RELATIVE_INCREASE_THRESHOLD:.0%} threshold."
            )
        elif delta < 0:
            print(f"OK     [relative] Size shrank by {_human(abs(delta))} ({abs(pct):.1%}).")
        else:
            print("OK     [relative] Size unchanged from baseline.")
    else:
        print("INFO   [relative] No baseline recorded yet — skipping growth check.")
        print("       Run with --record on a known-good build to set the baseline.")
        print()

    # --- Record if requested ---
    if record and not failed:
        history: list = baseline_data.get("history", [])
        if baseline_bytes is not None:
            history.append(
                {
                    "date": now,
                    "commit": git.get("commit", "unknown"),
                    "ref": git.get("ref", "unknown"),
                    "bytes": baseline_bytes,
                }
            )
        # Keep last 100 entries
        baseline_data["history"] = history[-100:]
        baseline_data["baseline_bytes"] = current_bytes
        baseline_data["last_updated"] = now
        baseline_data["last_commit"] = git.get("commit", "unknown")
        baseline_data["last_ref"] = git.get("ref", "unknown")
        save_baseline(baseline_path, baseline_data)
        print(f"Baseline updated → {baseline_path}  ({_human(current_bytes)})")
    elif record and failed:
        print(
            "WARNING: --record skipped because size checks failed. "
            "Fix the size regression first.",
            file=sys.stderr,
        )

    return 1 if failed else 0


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Check karis-ky escrow WASM artifact size against baseline."
    )
    parser.add_argument(
        "--wasm",
        default=DEFAULT_WASM,
        help=f"Path to WASM file (default: {DEFAULT_WASM})",
    )
    parser.add_argument(
        "--baseline",
        default=DEFAULT_BASELINE,
        help=f"Path to baseline JSON (default: {DEFAULT_BASELINE})",
    )
    parser.add_argument(
        "--record",
        action="store_true",
        help="Update the baseline with the current size after a passing check.",
    )
    args = parser.parse_args()

    exit_code = check_size(
        wasm_path=Path(args.wasm),
        baseline_path=Path(args.baseline),
        record=args.record,
    )
    # Non-zero exit code fails the CI job (hard failure, not just a report).
    sys.exit(exit_code)


if __name__ == "__main__":
    main()
