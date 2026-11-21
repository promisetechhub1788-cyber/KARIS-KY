# CI Coverage Threshold Issue — Quick Summary

**Issue:** Integration tests in `escrow/tests/` are excluded from coverage measurement, allowing untested code paths to bypass the 95% gate.

## The Problem in 30 Seconds

| Component | Status |
|-----------|--------|
| Unit tests (`src/test/*`) | ✅ Measured, 95% gate applied |
| Integration tests (`tests/*`) | ❌ Excluded, no gate applied |
| Coverage mismatch | CI command differs from README docs |

**Current CI command:**
```bash
cargo llvm-cov -p karis-ky_escrow[workspace] --fail-under-lines 95
```

**Workspace exclusion rule (Cargo.toml:11-14):**
```toml
exclude = ["src/test/*", "**/*_test.rs", "tests/*"]  # ← tests/* excludes integration tests
```

**Result:** Integration tests in `escrow/tests/simulation.rs` and `escrow/tests/snapshots.rs` run but are **not counted** toward the 95% coverage threshold.

## Why It Matters

- **Risk:** Integration test gaps (e.g., untested settlement flow) can go undetected.
- **Inconsistency:** Unit tests are strict (95% gate); integration tests are lenient (no gate).
- **Docs drift:** README documents a different (correct) command than CI actually runs.

## Files to Modify

1. `.github/workflows/ci.yml` (line 185): Update coverage command.
2. `Cargo.toml` (line 11–14): Remove `"tests/*"` from exclusions.
3. `README.md` (line 320): Verify alignment with updated command.
4. Create `docs/coverage-policy.md`: Document the decision.

## Recommended Fix

**Option A (Recommended):** Include integration tests in the 95% gate.

1. Change CI command from:
   ```bash
   cargo llvm-cov -p karis-ky_escrow[workspace] --fail-under-lines 95
   ```
   to:
   ```bash
   cargo llvm-cov -p karis-ky_escrow --fail-under-lines 95
   ```

2. Update `Cargo.toml` exclude list to:
   ```toml
   exclude = ["src/test/*", "**/*_test.rs"]
   ```

3. Document decision in `docs/coverage-policy.md`.

**Result:** Integration tests now measured and gated at 95%, matching unit tests.

## Verification Steps

1. Make the changes above.
2. Run locally: `cargo llvm-cov -p karis-ky_escrow --fail-under-lines 95 --summary-only`
3. Confirm integration tests are now included.
4. Add a test with unreachable code to `escrow/tests/simulation.rs` and verify CI fails.

## Full Specification

See: `BUG_CI_COVERAGE_THRESHOLD_INTEGRATION_TESTS.md`
