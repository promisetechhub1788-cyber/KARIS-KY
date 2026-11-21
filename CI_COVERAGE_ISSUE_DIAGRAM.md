# CI Coverage Threshold Issue — Visual Breakdown

## Current State (Broken)

```
┌─────────────────────────────────────────────────────────────────┐
│  KARIS-KY Workspace                                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Escrow Crate (karis_ky_escrow)                          │  │
│  ├──────────────────────────────────────────────────────────┤  │
│  │                                                          │  │
│  │  ┌─────────────────────────────────────────────────┐   │  │
│  │  │ src/                                            │   │  │
│  │  │  ├─ lib.rs (contract code)         ✅ MEASURED│   │  │
│  │  │  ├─ test.rs (test helpers)                    │   │  │
│  │  │  └─ test/                          ❌ EXCLUDED│   │  │
│  │  │     ├─ init.rs                    (scaffolding)   │  │
│  │  │     ├─ funding.rs                               │  │
│  │  │     ├─ settlement.rs                            │  │
│  │  │     ├─ admin.rs                                 │  │
│  │  │     ├─ integration.rs                           │  │
│  │  │     └─ properties.rs                            │  │
│  │  └─────────────────────────────────────────────────┘   │  │
│  │                                                          │  │
│  │  ┌─────────────────────────────────────────────────┐   │  │
│  │  │ tests/  (Integration Tests)      ❌ EXCLUDED   │   │  │
│  │  │  ├─ simulation.rs  (14+ scenarios)            │   │  │
│  │  │  │   └─ Untested code paths slip through!    │   │  │
│  │  │  └─ snapshots.rs  (state snapshots)          │   │  │
│  │  └─────────────────────────────────────────────────┘   │  │
│  │                                                          │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

CI Command (line .github/workflows/ci.yml:185):
  cargo llvm-cov -p karis-ky_escrow[workspace] --fail-under-lines 95

Workspace Metadata (Cargo.toml:9-14):
  exclude = ["src/test/*", "**/*_test.rs", "tests/*"]
                                            ↑↑↑↑↑↑↑
                                   This excludes integration tests!

Coverage Gate Result:
  ✅ PASS (only src/ measured, tests/ ignored)
  ⚠️  False confidence: integration test gaps undetected
```

---

## Expected State (Fixed — Option A)

```
┌─────────────────────────────────────────────────────────────────┐
│  KARIS-KY Workspace                                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Escrow Crate (karis_ky_escrow)                          │  │
│  ├──────────────────────────────────────────────────────────┤  │
│  │                                                          │  │
│  │  ┌─────────────────────────────────────────────────┐   │  │
│  │  │ src/                                            │   │  │
│  │  │  ├─ lib.rs (contract code)         ✅ MEASURED│   │  │
│  │  │  ├─ test.rs (test helpers)                    │   │  │
│  │  │  └─ test/                          ❌ EXCLUDED│   │  │
│  │  │     ├─ init.rs                    (scaffolding)   │  │
│  │  │     ├─ funding.rs                               │  │
│  │  │     ├─ settlement.rs                            │  │
│  │  │     ├─ admin.rs                                 │  │
│  │  │     ├─ integration.rs                           │  │
│  │  │     └─ properties.rs                            │  │
│  │  └─────────────────────────────────────────────────┘   │  │
│  │                                                          │  │
│  │  ┌─────────────────────────────────────────────────┐   │  │
│  │  │ tests/  (Integration Tests)      ✅ MEASURED   │   │  │
│  │  │  ├─ simulation.rs  (14+ scenarios)            │   │  │
│  │  │  │   └─ All paths verified against gate       │   │  │
│  │  │  └─ snapshots.rs  (state snapshots)          │   │  │
│  │  └─────────────────────────────────────────────────┘   │  │
│  │                                                          │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

CI Command (updated line .github/workflows/ci.yml:185):
  cargo llvm-cov -p karis-ky_escrow --fail-under-lines 95
                                     ↑ Remove [workspace]

Workspace Metadata (updated Cargo.toml:9-14):
  exclude = ["src/test/*", "**/*_test.rs"]
            ↑ Remove "tests/*" ↑

Coverage Gate Result:
  ✅ PASS (only if unit + integration reach ≥95%)
  ✅ Strong guarantee: both test types verified
```

---

## Comparison Table

| Aspect | Current (Broken) | Fixed (Option A) |
|--------|------------------|-----------------|
| **Unit tests measured** | ✅ Yes | ✅ Yes |
| **Integration tests measured** | ❌ No | ✅ Yes |
| **Combined coverage gate** | Unit only | Unit + Integration |
| **Documentation match** | ❌ Drift | ✅ Aligned |
| **Risk of unmeasured paths** | ⚠️ High | ✅ None |

---

## File Changes Required

### 1. `.github/workflows/ci.yml` (line 185)

**Before:**
```yaml
- name: Check coverage (≥ 95%)
  run: |
    cargo llvm-cov \
      --features testutils \
      --fail-under-lines 95 \
      --summary-only \
      -p karis-ky_escrow[workspace]     ← Remove [workspace]
```

**After:**
```yaml
- name: Check coverage (≥ 95%)
  run: |
    cargo llvm-cov \
      --features testutils \
      --fail-under-lines 95 \
      --summary-only \
      -p karis-ky_escrow               ← Without [workspace]
```

### 2. `Cargo.toml` (line 11–14)

**Before:**
```toml
[workspace.metadata.cargo-llvm-cov]
exclude = [
    "src/test/*",
    "**/*_test.rs",
    "tests/*"              ← Remove this line
]
```

**After:**
```toml
[workspace.metadata.cargo-llvm-cov]
exclude = [
    "src/test/*",          # Unit test scaffolding only
    "**/*_test.rs"
]
```

### 3. `README.md` (line 320) — No change needed

Already documents the correct command:
```bash
cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only -p karis-ky_escrow
```

---

## Coverage Impact Illustration

### Before (Hypothetical Scenario)

```
Integration Tests (not measured):
  ┌─────────────────────────────┐
  │ 50% coverage (unmeasured)   │ ← Gap!
  │ - settlement_flow           │
  │ - multi_escrow_scenario    │
  └─────────────────────────────┘

Unit Tests (measured):
  ┌─────────────────────────────┐
  │ 96% coverage (measured)     │ ✓ Passes gate
  │ - init tests                │
  │ - fund tests                │
  └─────────────────────────────┘

Combined Report:
  "Coverage: 96% ✓ PASS"
  ⚠️  Misleading! Integration tests not counted.
```

### After (Fixed)

```
Integration Tests (now measured):
  ┌─────────────────────────────┐
  │ 50% coverage (measured)     │ ✗ Below 95%
  │ - settlement_flow           │
  │ - multi_escrow_scenario    │
  └─────────────────────────────┘

Unit Tests (measured):
  ┌─────────────────────────────┐
  │ 96% coverage (measured)     │ ✓ Passes gate
  │ - init tests                │
  │ - fund tests                │
  └─────────────────────────────┘

Combined Report:
  "Coverage: 79% ✗ FAIL"
  ✓ Accurate! Identifies integration test gaps.

Developer Action:
  → Write tests to reach ≥95% coverage
```

---

## Test Scenarios for Verification

### Scenario 1: Verify Fix Catches Unmeasured Code

Add this to `escrow/tests/simulation.rs`:
```rust
#[test]
fn integration_test_with_unreachable_code() {
    // This represents untested code path
    if false {
        panic!("Unreachable!");  // ← Will lower coverage
    }
}
```

**Before fix:** ✅ CI passes (test not measured)  
**After fix:** ❌ CI fails (coverage drops below 95%)  
**Expected:** This verifies the gate is now working.

### Scenario 2: Verify Documented Command Matches CI

Run both locally:
```bash
# README command
cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only -p karis-ky_escrow

# CI command (after fix)
cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only -p karis-ky_escrow
```

**Expected:** Same results (commands are identical).

---

## Key Takeaway

**The issue:** Integration tests are silently excluded from coverage measurement, creating a false sense of security.

**The fix:** Include integration tests in the 95% gate by:
1. Removing `[workspace]` from CI command
2. Removing `"tests/*"` from workspace exclude list
3. Documenting the decision

**The outcome:** Comprehensive coverage guarantee for both unit and integration tests.
