# BUG: CI Coverage Threshold Not Applied to Integration Test Crate

**Issue ID:** CI-COVERAGE-001  
**Category:** BUG  
**Status:** Needs full specification  
**Priority:** Medium  
**Component:** CI/CD · Code Coverage  
**Date Created:** 2026-08-28  

---

## Executive Summary

The CI workflow's code coverage gate (`cargo llvm-cov --fail-under-lines 95`) only enforces the 95% line coverage threshold on the main `karis_ky_escrow` library crate via the `-p karis-ky_escrow[workspace]` flag. Integration tests located in `escrow/tests/` (e.g., `simulation.rs`, `snapshots.rs`) execute during `cargo test` but are **excluded from coverage measurement**, allowing them to have unmeasured test code paths. This creates a coverage blind spot and inconsistency with the project's documented CI standard.

---

## Full Description

### What is happening

**Current CI behavior (.github/workflows/ci.yml:185):**
```yaml
- name: Check coverage (≥ 95%)
  run: |
    cargo llvm-cov \
      --features testutils \
      --fail-under-lines 95 \
      --summary-only \
      -p karis-ky_escrow[workspace]
```

**Workspace configuration (Cargo.toml:9-14):**
```toml
[workspace.metadata.cargo-llvm-cov]
# Exclude test scaffolding from coverage measurement — only production code is gated.
exclude = [
    "src/test/*",
    "**/*_test.rs",
    "tests/*"
]
```

**Issue:** The workspace-level exclusion rule `"tests/*"` blanket-excludes all integration tests in the `tests/` directory tree from coverage reporting. The CI command does not override this exclusion, so integration tests in `escrow/tests/` are never measured against the 95% threshold.

### Test Code Structure

The project has two categories of tests:

| Category | Location | Purpose | Coverage Status |
|----------|----------|---------|-----------------|
| **Unit tests** | `escrow/src/test/` (init.rs, funding.rs, settlement.rs, admin.rs, integration.rs, properties.rs) | Feature area tests; bundled into library crate | ✅ Measured (95% gate applied) |
| **Integration tests** | `escrow/tests/` (simulation.rs, snapshots.rs) | Standalone multi-file scenarios; separate crate | ❌ **Excluded (no gate)** |

**Documented expectation (README.md:320):**
```bash
cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only -p karis-ky_escrow
```

The command in the README does not use `[workspace]`, which would measure integration tests by default.

### Why This Is a Problem

1. **Blind spot:** Integration tests can grow without coverage verification, contradicting the project's 95% minimum standard.
2. **Inconsistent enforcement:** Unit tests are gated at 95%; integration tests are not.
3. **Documentation drift:** The README documents a different (stricter) command than CI actually runs.
4. **Risk:** Untested integration test paths in production codepaths (e.g., multi-escrow settlement scenarios in `simulation.rs`) can slip through.

---

## Steps to Reproduce

### Scenario 1: Verify Current Coverage Gap

1. Clone the repository:
   ```bash
   git clone <repo-url>
   cd KARIS-KY
   ```

2. Install cargo-llvm-cov (if not present):
   ```bash
   cargo install cargo-llvm-cov
   ```

3. Add a trivial, untested code path to `escrow/tests/simulation.rs`:
   ```rust
   #[test]
   fn never_called_integration_test() {
       panic!("This test never runs but will never be called");
   }
   ```

4. Run the CI coverage command as-is:
   ```bash
   cargo llvm-cov \
     --features testutils \
     --fail-under-lines 95 \
     --summary-only \
     -p karis-ky_escrow[workspace]
   ```

   **Expected (current broken behavior):** Coverage passes ≥95% (integration test not counted).  
   **Expected (fixed behavior):** Command would need to be adjusted to include integration tests.

5. Run the README command to compare:
   ```bash
   cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only -p karis-ky_escrow
   ```

   **Observation:** May show different coverage or fail if integration tests have gaps.

### Scenario 2: Measure Integration Test Coverage Separately

1. Measure just integration test coverage (excluding `src/test/`):
   ```bash
   cargo llvm-cov \
     --features testutils \
     --summary-only \
     --ignore-filename-regex "src/test/" \
     -p karis-ky_escrow
   ```

   **Observation:** Integration tests in `escrow/tests/` are now included. If any are uncovered, the percentage drops below 95%.

---

## Expected vs. Actual Behavior

| Aspect | Expected (Per Docs) | Actual (Current CI) |
|--------|-------------------|-------------------|
| **Coverage scope** | All code: unit tests + integration tests | Only unit tests |
| **Gate applied** | 95% threshold on combined coverage | 95% threshold on unit tests only |
| **Integration test measurement** | Included (per README command) | Excluded (per workspace.metadata) |
| **CI pass/fail** | Strict; fails if any test type drops below 95% | Lenient; integration test gaps pass silently |

---

## Environment Context

- **OS:** Linux (ubuntu-latest in CI)
- **Rust:** Stable (with llvm-tools-preview component)
- **Project:** karis-ky escrow contract (Soroban/Stellar smart contract)
- **Workspace:** Single-package workspace: `escrow`
- **CI Platform:** GitHub Actions
- **Tool:** cargo-llvm-cov (version: latest from Swatinem/rust-cache action)

### Crate and Test Layout

```
KARIS-KY/
├── Cargo.toml (workspace)
├── escrow/
│   ├── Cargo.toml (lib crate)
│   ├── src/
│   │   ├── lib.rs
│   │   ├── test.rs (shared test helpers)
│   │   └── test/
│   │       ├── init.rs
│   │       ├── funding.rs
│   │       ├── settlement.rs
│   │       ├── admin.rs
│   │       ├── integration.rs
│   │       └── properties.rs
│   └── tests/
│       ├── simulation.rs (integration test — separate crate)
│       └── snapshots.rs (integration test — separate crate)
└── .github/workflows/ci.yml
```

**Unit tests in `src/test/`:** Run as part of `cargo test -p karis_ky_escrow`. Included in coverage by default.  
**Integration tests in `tests/`:** Run as part of `cargo test -p karis_ky_escrow`. Normally included in coverage, but **excluded by workspace.metadata**.

---

## Acceptance Criteria

### AC-1: Clarify Coverage Scope in CI

- [ ] Decision made: Should integration tests be included in the 95% gate or kept separate?
  - **Option A (Recommended):** Include integration tests in coverage; apply 95% gate to both.
  - **Option B:** Explicitly exclude integration tests; document the decision and set a separate (lower) target or no target.

### AC-2: Update CI Workflow to Match Decision

- [ ] If **Option A:**
  - [ ] Modify `.github/workflows/ci.yml` line 185 to remove `[workspace]` and/or update the exclude list to not blanket-exclude `tests/*`.
  - [ ] Verify the command runs with no `[workspace]` specifier (so `cargo llvm-cov` picks up all crates, including integration tests).
  - [ ] Confirm integration tests are now included in the coverage report.
  - [ ] CI passes with ≥95% coverage on combined unit + integration tests.

- [ ] If **Option B:**
  - [ ] Document the decision in `docs/coverage-policy.md`.
  - [ ] Add a separate CI step that measures integration test coverage separately (report-only, no gate).
  - [ ] Update the workspace.metadata to clarify the rationale for excluding `tests/*`.

### AC-3: Ensure Documentation Consistency

- [ ] Update `.github/workflows/ci.yml` with an inline comment explaining the coverage scope (unit tests, integration tests, or both).
- [ ] Update `README.md` to match the actual CI command (currently it documents a command that differs from what runs).
- [ ] Add a new section to `docs/coverage-policy.md` (or similar) documenting:
  - Coverage targets (unit tests, integration tests).
  - Exclusion rules (why `src/test/*` is excluded from counting).
  - How developers should verify coverage locally.

### AC-4: Test the Fix

- [ ] Run the updated CI command locally and confirm integration tests are now measured (or separately reported, if Option B).
- [ ] Add a failing test to `escrow/tests/` that intentionally has an untested code path (e.g., `unreachable!()` in a branch) to verify the gate catches it.
- [ ] Confirm CI fails as expected due to coverage threshold breach.
- [ ] Remove the failing test; CI passes again.

### AC-5: Communicate Rationale to Team

- [ ] If changing coverage scope, document in PR description why the change matters.
- [ ] Update `CONTRIBUTING.md` (if it exists) or add a section to `docs/CONTRIBUTING.md` on coverage expectations.

---

## Proposed Solution

### Recommended Approach: **Option A** (Include Integration Tests)

**Rationale:**
- Integration tests (`escrow/tests/`) verify end-to-end contract flows that are critical to correctness.
- Excluding them from coverage creates a false confidence; a bug in settlement flow tested only by `simulation.rs` would go unmeasured.
- The project's documented standard is 95% line coverage — should apply uniformly.
- Unit tests in `src/test/` are excluded from **counting** (but not from **running**) because they're scaffolding; integration tests are real scenarios.

### Implementation Steps

1. **Update `.github/workflows/ci.yml` (line 185):**
   ```yaml
   - name: Check coverage (≥ 95%)
     run: |
       cargo llvm-cov \
         --features testutils \
         --fail-under-lines 95 \
         --summary-only \
         -p karis-ky_escrow
   ```
   *(Remove `[workspace]` to include integration tests by default.)*

2. **Verify workspace metadata is correct (Cargo.toml:11-14):**
   ```toml
   [workspace.metadata.cargo-llvm-cov]
   # Exclude unit-test scaffolding from line coverage only; integration tests and 
   # contract code must maintain ≥95% line coverage.
   exclude = [
       "src/test/*",          # Unit test helpers (scaffolding, not production logic)
       "**/*_test.rs"
   ]
   ```
   *(Remove `"tests/*"` so integration tests in `escrow/tests/` are measured.)*

3. **Update `README.md` (line 320):**
   Keep the documented command as-is (it already matches the correct behavior):
   ```bash
   cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only -p karis-ky_escrow
   ```

4. **Add new documentation file `docs/coverage-policy.md`:**
   - Explain the coverage model: unit tests + integration tests both measured.
   - Document why `src/test/*` is excluded (scaffolding).
   - Explain how to run coverage locally.
   - Provide examples of coverage reports.

5. **Update `CONTRIBUTING.md` (or create if missing):**
   - Add section: "Code Coverage Requirements"
   - State: "All code (unit and integration tests) must maintain ≥95% line coverage."
   - Link to `docs/coverage-policy.md`.

---

## Alternative: Option B (Separate Integration Test Coverage)

If the team decides integration tests should **not** be gated at 95% (e.g., they're harder to reach 95% on), then:

1. Update CI to run coverage in two steps:
   ```yaml
   - name: Check unit test coverage (≥ 95%)
     run: |
       cargo llvm-cov \
         --features testutils \
         --fail-under-lines 95 \
         --summary-only \
         --ignore-filename-regex "tests/" \
         -p karis-ky_escrow

   - name: Report integration test coverage (informational)
     run: |
       cargo llvm-cov \
         --features testutils \
         --summary-only \
         --ignore-filename-regex "src/" \
         -p karis-ky_escrow
   ```

2. Document the decision in `docs/coverage-policy.md`.

---

## Impact Assessment

### What changes:

- **CI behavior:** Integration tests now measured against 95% gate (if Option A).
- **Coverage reports:** Will show combined coverage (unit + integration).
- **Developer experience:** May need to write more tests for integration test scenarios to reach 95%.

### What does NOT change:

- **Unit test execution:** Still runs, still measured.
- **WASM build, audit, spec publishing:** Unaffected.
- **Local development:** `cargo test` works as before; coverage measurement uses new command.

### Risk level: **Low**

- Change is in CI configuration only.
- No contract logic changes.
- Worst case: CI fails if integration tests have coverage gaps (desired outcome).

---

## Acceptance Sign-Off Checklist

- [ ] Coverage scope decision made and documented (Option A or B).
- [ ] CI workflow updated and tested locally.
- [ ] `README.md` and documentation consistent with CI behavior.
- [ ] PR includes failing test scenario to validate the fix.
- [ ] Team agrees on coverage policy for integration tests.
- [ ] PR reviewed and approved.

---

## Related Issues / Tickets

- **CI-related:**
  - `.github/workflows/ci.yml` line 185
  - Workspace metadata in `Cargo.toml` line 9–14

- **Documentation:**
  - `README.md` line 320
  - `escrow/README.md` line 129, 137–151
  - `docs/OPERATOR_RUNBOOK.md` section "Release runbook"

- **Tests that should be measured:**
  - `escrow/tests/simulation.rs` (14+ test cases)
  - `escrow/tests/snapshots.rs` (snapshot validation)

---

## Questions for Clarification

1. **Design intent:** Were integration tests intentionally excluded from coverage, or is this an oversight?
2. **Coverage target:** Should integration tests also target ≥95%, or is a lower threshold acceptable?
3. **Maintenance burden:** If 95% is required for integration tests, are there existing gaps that need to be fixed before enabling the gate?
4. **Timeline:** Can this be addressed in the next sprint, or should it be backlogged?

---

## Developer Notes

- Run coverage locally with: `cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only -p karis-ky_escrow`
- See existing docs: `escrow/LOCAL_REPRODUCTION.md`, `TEST_RUNNER_GUIDE.md`
- Coverage tool: [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)
- CI step runs on: `ubuntu-latest` with stable Rust + llvm-tools-preview

---

## References

- **Workspace metadata:** https://doc.rust-lang.org/cargo/reference/manifest.html#workspace
- **cargo-llvm-cov docs:** https://github.com/taiki-e/cargo-llvm-cov
- **Project README:** `README.md` (line 320)
- **Local setup:** `escrow/LOCAL_REPRODUCTION.md`
