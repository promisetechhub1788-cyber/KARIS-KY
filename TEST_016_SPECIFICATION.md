# TEST-016: bind_primary_attestation_hash — Single-Write Immutability

**Issue ID:** TEST-016  
**Category:** TEST  
**Status:** Ready for Assignment  
**Priority:** HIGH (Security-critical)  
**Component:** Escrow Attestation System  

---

## 1. FULL DESCRIPTION

### Overview
The `bind_primary_attestation_hash()` function must enforce strict single-write immutability on the primary attestation digest stored in the escrow contract. Once a 32-byte SHA-256 hash is bound by an admin, it must become immutable and unreplaceable for the lifetime of the escrow. The function must reject any subsequent binding attempts with a deterministic error, preventing race conditions or accidental overwrites.

### Purpose
This test ensures that the primary attestation hash—used as a compliance anchor for KYC/KYB document bundles—cannot be modified or replaced after the initial binding. This is a critical security invariant (I-5) because:
- **Immutability guarantee**: Legal and regulatory requirements demand that evidence fingerprints remain tamper-proof
- **Race-condition prevention**: In concurrent multi-contract environments, "first-write-wins" semantics must be enforced deterministically
- **Audit trail integrity**: The hash serves as an on-chain audit anchor; replacement would break compliance chains
- **No bypass paths**: The function must fail clearly rather than silently accepting duplicates or allowing overwrites

### Technical Context
- **Function location:** [escrow/src/lib.rs](escrow/src/lib.rs#L3206-L3240)
- **Storage key:** `DataKey::PrimaryAttestationHash`
- **Hash format:** BytesN<32> (exactly 32 bytes, e.g., SHA-256)
- **Authorization:** Admin-only via `escrow.admin.require_auth()`
- **Event:** Emits `PrimaryAttestationBound` with invoice_id, digest, and event name
- **Related fields:** 
  - `FundingCloseSnapshot` (single-write)
  - `FundingToken` (immutable after init)
  - `Treasury` (immutable after init)
  - Follows same single-write pattern as `InvestorClaimed(addr)`

---

## 2. STEPS TO REPRODUCE / PROPOSED SOLUTION

### 2.1 Test Scenario A: Happy Path—Initial Binding (PASS)
**Goal:** Verify successful first-time binding of a valid attestation digest.

**Steps:**
1. Create an escrow contract with valid admin credentials
2. Prepare a 32-byte digest (valid SHA-256 hash, e.g., `[0u8; 32]`)
3. Call `bind_primary_attestation_hash(env, digest)` as the admin
4. Read the binding result via `get_primary_attestation_hash()` storage query
5. Verify the event `PrimaryAttestationBound` was emitted with correct data

**Expected behavior:**
- ✅ Function returns without panic
- ✅ Storage read returns `Some(digest)` matching the bound value
- ✅ Event published with `name: "att_bind"`, `invoice_id` from escrow, and exact digest
- ✅ Transaction succeeds with no errors

**Actual behavior:** (To be filled after test execution)

---

### 2.2 Test Scenario B: Second Binding with Different Digest (FAIL)
**Goal:** Verify that a second bind call with a *different* digest is rejected.

**Steps:**
1. Execute Test Scenario A to bind an initial digest (e.g., `digest_1 = [0u8; 32]`)
2. Prepare a different 32-byte digest (e.g., `digest_2 = [1u8; 32]`)
3. Call `bind_primary_attestation_hash(env, digest_2)` as the same admin
4. Observe the error returned/panic thrown

**Expected behavior:**
- ❌ Function panics or returns error code `53` (`AttestationHashAlreadyBound`)
- ❌ Storage remains unchanged (still contains `digest_1`)
- ❌ No event is emitted on failure
- ❌ Second call completely fails before any write occurs

**Actual behavior:** (To be filled after test execution)

---

### 2.3 Test Scenario C: Second Binding with Identical Digest (FAIL)
**Goal:** Verify that even an idempotent second bind (same digest) is rejected.

**Steps:**
1. Execute Test Scenario A to bind a digest (e.g., `digest = [42u8; 32]`)
2. Call `bind_primary_attestation_hash(env, digest)` again with the *same* digest
3. Observe the error returned/panic thrown

**Expected behavior:**
- ❌ Function panics or returns error code `53` (`AttestationHashAlreadyBound`)
- ❌ No event is emitted on second attempt
- ❌ Rejection is **not** based on digest equality (same digest still fails)
- ❌ This differs from idempotent patterns; second call always fails

**Actual behavior:** (To be filled after test execution)

**Rationale:** Idempotent patterns (e.g., investor claim resubmission) are allowed for atomicity. However, `bind_primary_attestation_hash` must fail on every second call to provide a clear signal that the operation completed or was already done. This prevents confusion in retry loops.

---

### 2.4 Test Scenario D: Authorization Enforcement (FAIL)
**Goal:** Verify that non-admin callers cannot bind attestation hashes.

**Steps:**
1. Create an escrow contract with admin credentials
2. Switch to a non-admin user (or unsigned invocation)
3. Prepare a valid 32-byte digest
4. Call `bind_primary_attestation_hash(env, digest)` as the non-admin user
5. Observe the authorization failure

**Expected behavior:**
- ❌ Function panics with authorization/authentication error before validation
- ❌ No write to storage occurs
- ❌ Error occurs during `escrow.admin.require_auth()` check, not after
- ❌ Non-admin cannot bypass immutability checks

**Actual behavior:** (To be filled after test execution)

---

### 2.5 Test Scenario E: Invalid Digest Length—31 Bytes (FAIL)
**Goal:** Verify rejection of undersized digests.

**Steps:**
1. Create an escrow contract with valid admin credentials
2. Prepare a 31-byte digest (one byte short)
3. Call `bind_primary_attestation_hash(env, digest)` as the admin
4. Observe the validation error

**Expected behavior:**
- ❌ Function panics or returns error code `52` (`InvalidAttestationHashLength`)
- ❌ Error is raised **before** any storage write or authorization check
- ❌ Validation message indicates expected 32 bytes vs. actual 31 bytes
- ❌ No storage state is modified

**Actual behavior:** (To be filled after test execution)

---

### 2.6 Test Scenario F: Invalid Digest Length—33 Bytes (FAIL)
**Goal:** Verify rejection of oversized digests.

**Steps:**
1. Create an escrow contract with valid admin credentials
2. Prepare a 33-byte digest (one byte too many)
3. Call `bind_primary_attestation_hash(env, digest)` as the admin
4. Observe the validation error

**Expected behavior:**
- ❌ Function panics or returns error code `52` (`InvalidAttestationHashLength`)
- ❌ Rejection occurs during length validation, not type conversion
- ❌ Validation is strict (not "at least 32" or "up to 32")
- ❌ No storage modification

**Actual behavior:** (To be filled after test execution)

---

### 2.7 Test Scenario G: Empty Digest (FAIL)
**Goal:** Verify rejection of zero-length digests.

**Steps:**
1. Create an escrow contract with valid admin credentials
2. Prepare an empty byte array (0 bytes)
3. Call `bind_primary_attestation_hash(env, digest)` with the empty array
4. Observe the validation error

**Expected behavior:**
- ❌ Function rejects with error code `52` (`InvalidAttestationHashLength`)
- ❌ Validation catches the zero-length case
- ❌ No state modification

**Actual behavior:** (To be filled after test execution)

---

### 2.8 Test Scenario H: Pre-Read (None Before Binding)
**Goal:** Verify that the attestation hash is unset before initial binding.

**Steps:**
1. Create a fresh escrow contract
2. Call `get_primary_attestation_hash()` to read the hash state
3. Attempt to bind a digest
4. Re-read the state after binding

**Expected behavior:**
- ✅ Pre-bind read returns `None` (Option::None or equivalent)
- ✅ Post-bind read returns `Some(digest)`
- ✅ Default state is empty/unset
- ✅ Binding transitions the state once

**Actual behavior:** (To be filled after test execution)

---

## 3. EXPECTED VS. ACTUAL BEHAVIOUR

### Expected Behavior Matrix

| Scenario | Input | Pre-Condition | Action | Expected Outcome | Error Code | Storage State |
|----------|-------|----------------|--------|------------------|-----------|----------------|
| **A** Happy Path | digest=32B | Unset | bind_primary() | ✅ Success, emit event | — | digest stored |
| **B** Different 2nd Bind | digest_2=32B | digest_1 set | bind_primary() | ❌ Panic/reject | 50 | digest_1 unchanged |
| **C** Identical 2nd Bind | digest=32B | digest set | bind_primary() | ❌ Panic/reject | 50 | digest unchanged |
| **D** Non-Admin Caller | digest=32B | Unset, caller ≠ admin | bind_primary() | ❌ Auth error | 4 (assumed) | Unset |
| **E** Undersized (31B) | digest=31B | Unset | bind_primary() | ❌ Validation error | 52 | Unset |
| **F** Oversized (33B) | digest=33B | Unset | bind_primary() | ❌ Validation error | 52 | Unset |
| **G** Empty (0B) | digest=0B | Unset | bind_primary() | ❌ Validation error | 52 | Unset |
| **H** Pre-Read | — | Fresh escrow | read() | ✅ None | — | Unset |

### Actual Behavior
*(To be filled after running test suite)*

---

## 4. ENVIRONMENT CONTEXT

### Test Environment Setup
- **Language/Framework:** Rust + Soroban SDK
- **Test runner:** `cargo test` with optional `[test]` feature gates
- **Contract:** `liquifact-escrow` (liquiverse escrow module)
- **Test suite location:** [escrow/src/tests/attestations.rs](escrow/src/tests/attestations.rs)
- **Related test file:** [escrow/src/tests/coverage.rs](escrow/src/tests/coverage.rs) (error code validation)

### Dependencies
- `soroban_sdk` — Contract SDK and host functions
- `soroban_sdk::testutils` — Test contract environment (`Env::default()`)
- Contract error enums (see [escrow/src/lib.rs](escrow/src/lib.rs#L320-L325))

### Attestation Module Architecture
```
┌─────────────────────────────────────────────────────────┐
│ Escrow Contract (liquifact-escrow)                       │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌─ bind_primary_attestation_hash(digest) ────────┐   │
│  │  • 32-byte SHA-256 hash binding                  │   │
│  │  • Single-write immutable (first-write-wins)    │   │
│  │  • Admin-only authorization                     │   │
│  │  • Emits PrimaryAttestationBound event          │   │
│  └───────────────────────────────────────────────┘   │
│                                                          │
│  ┌─ append_attestation_digest(digest) ───────────┐   │
│  │  • Append-only log (bounded at 32 entries)     │   │
│  │  • Multiple hashes allowed                     │   │
│  └───────────────────────────────────────────────┘   │
│                                                          │
│  ┌─ revoke_attestation_digest(index) ────────────┐   │
│  │  • Mark entry superseded (single-write per i)  │   │
│  └───────────────────────────────────────────────┘   │
│                                                          │
│ Storage: DataKey::PrimaryAttestationHash               │
│          DataKey::AttestationAppendLog                 │
│          DataKey::AttestationRevoked(index)            │
└─────────────────────────────────────────────────────────┘
```

### Security Context
- **Invariant I-5:** `bind_primary_attestation_hash` enforces single-set immutability
- **Compliance:** KYC/KYB document bundle binding (off-chain verification required)
- **Threat model:** 
  - Admin accidentally re-binding (prevented by error)
  - Race conditions in multi-contract scenarios (prevented by storage guard)
  - Unauthorized binding (prevented by authorization check)
  - Corrupted digest on-chain (prevented by length validation)

### Related Test Files
- [escrow/src/tests/attestations.rs](escrow/src/tests/attestations.rs) — Existing tests (8 tests already implemented)
- [escrow/src/tests/coverage.rs](escrow/src/tests/coverage.rs#L407-L410) — Error code coverage

---

## 5. ACCEPTANCE CRITERIA

### Core Requirements (Must Pass)

#### AC-1: Single-Write Enforcement
- [ ] First successful binding stores digest in `DataKey::PrimaryAttestationHash` ✅
- [ ] Subsequent bind attempts with any digest (same or different) fail with error code `50` ✅
- [ ] Storage remains unchanged after rejection ✅
- [ ] No partial writes occur on failure ✅
- [ ] Rejection is deterministic (same input, same result) ✅

#### AC-2: Authorization Enforcement
- [ ] Admin can bind attestation hash ✅
- [ ] Non-admin caller is rejected before validation ✅
- [ ] Unsigned invocation is rejected ✅
- [ ] Authorization check occurs before storage modification ✅

#### AC-3: Input Validation
- [ ] Digest must be exactly 32 bytes ✅
- [ ] Undersized digest (< 32 bytes) rejected with error code `52` ✅
- [ ] Oversized digest (> 32 bytes) rejected with error code `52` ✅
- [ ] Empty digest rejected with error code `52` ✅
- [ ] Validation is strict (no rounding or truncation) ✅
- [ ] Validation occurs before authorization check ✅

#### AC-4: Event Emission
- [ ] `PrimaryAttestationBound` event emitted on successful binding ✅
- [ ] Event contains correct `invoice_id` ✅
- [ ] Event contains exact digest (same bytes) ✅
- [ ] Event name is `"att_bind"` (symbol_short) ✅
- [ ] No event on failed attempts ✅

#### AC-5: State Consistency
- [ ] Default state before binding returns `None` (Option::None) ✅
- [ ] After binding, state returns `Some(digest)` ✅
- [ ] State persists across multiple queries ✅
- [ ] State is recovered correctly after panic/error ✅

#### AC-6: Immutability Guarantee
- [ ] Storage key guard check: `!has(&DataKey::PrimaryAttestationHash)` verified ✅
- [ ] Second binding attempt triggers guard before any write ✅
- [ ] Error code `53` message: `"AttestationHashAlreadyBound"` ✅
- [ ] Invariant I-5 is preserved ✅

### Integration Requirements (Should Pass)

#### AC-7: Compatibility with Attestation Log
- [ ] `bind_primary_attestation_hash` does not interfere with `append_attestation_digest` ✅
- [ ] Both functions can coexist in same escrow ✅
- [ ] Primary hash is independent of append log ✅

#### AC-8: Event Publishing Reliability
- [ ] Events are published to the contract event log ✅
- [ ] Event can be queried via Soroban host environment ✅
- [ ] Event format matches `PrimaryAttestationBound` struct ✅

#### AC-9: Concurrency & Race Conditions
- [ ] Multiple concurrent bind attempts result in exactly one success ✅
- [ ] Losing threads all receive error code `50` ✅
- [ ] No ambiguous or partial states ✅

#### AC-10: Gas & Performance
- [ ] Single bind completes within typical gas budget ✅
- [ ] Rejected bind does not waste gas on unnecessary operations ✅
- [ ] Storage guard check is efficient (no full list scans) ✅

### Error Case Coverage (Must Reject)

#### AC-11: Error Codes
- [ ] Error code `53`: `AttestationHashAlreadyBound` — second bind attempt ✅
- [ ] Error code `52`: `InvalidAttestationHashLength` — wrong digest size ✅
- [ ] Error code `4` (assumed): Authorization failure — non-admin caller ✅
- [ ] All errors are typed (not opaque panics) ✅

#### AC-12: No Bypass Paths
- [ ] No function to clear/reset the hash ✅
- [ ] No function to replace the hash ✅
- [ ] No super-admin or override mechanism ✅
- [ ] No emergency pause that affects immutability ✅

### Documentation & Test Quality (Must Verify)

#### AC-13: Test Coverage
- [ ] All 8 scenarios (A–H) pass ✅
- [ ] Test names are descriptive (e.g., `test_bind_primary_hash_already_bound_panics`) ✅
- [ ] Test comments explain the invariant being verified ✅
- [ ] Test uses helper functions or fixtures for escrow setup ✅

#### AC-14: Error Message Clarity
- [ ] Error messages are user-friendly ✅
- [ ] Error codes are documented in inline comments ✅
- [ ] Error documentation references security invariant I-5 ✅

#### AC-15: Compliance & Auditability
- [ ] Comments reference docs/escrow-attestations.md ✅
- [ ] Rationale for single-write design is documented ✅
- [ ] Off-chain verification requirement is noted ✅

---

## 6. ASSIGNMENT NOTES

### Prior Art & Related Tests
This test expands on existing tests in [escrow/src/tests/attestations.rs](escrow/src/tests/attestations.rs):
- `test_bind_primary_hash_stores_and_reads` (line 47)
- `test_get_primary_hash_none_before_bind` (line 54)
- `test_bind_primary_hash_same_digest_panics` (line 62)
- `test_bind_primary_hash_different_digest_panics` (line 72)
- `test_bind_primary_hash_second_call_fails_with_primary_attestation_already_bound` (line 82)
- `test_bind_primary_hash_non_admin_panics` (line 94)
- `test_bind_primary_hash_31_bytes_rejected` (line 104)
- `test_bind_primary_hash_33_bytes_rejected` (line 114)

**Goal:** Consolidate, systematize, and document these tests into a coherent specification for traceability and maintenance.

### Implementation Checklist
- [ ] Read [escrow/src/lib.rs](escrow/src/lib.rs#L3206-L3240) to understand current implementation
- [ ] Review [escrow/src/tests/attestations.rs](escrow/src/tests/attestations.rs) for existing test patterns
- [ ] Verify all scenarios A–H are covered by tests
- [ ] Run `cargo test --lib escrow` and confirm all pass
- [ ] Document any deviations from expected behavior
- [ ] Update test comments with invariant references
- [ ] Validate error codes match [escrow/src/lib.rs](escrow/src/lib.rs#L320-L325)
- [ ] Sign off on AC-1 through AC-15

### Success Criteria
✅ All 15 Acceptance Criteria pass  
✅ All 8 test scenarios execute without panic  
✅ Error codes `50` and `52` are emitted correctly  
✅ Single-write immutability is verified  
✅ Specification document is complete and auditable  

---

## 7. REFERENCES

| Document | Purpose |
|----------|---------|
| [escrow/src/lib.rs](escrow/src/lib.rs#L3206-L3240) | Function implementation |
| [escrow/src/lib.rs](escrow/src/lib.rs#L320-L325) | Error code definitions |
| [escrow/src/lib.rs](escrow/src/lib.rs#L550-L620) | DataKey enum & storage structure |
| [escrow/src/tests/attestations.rs](escrow/src/tests/attestations.rs) | Existing test suite |
| [escrow/src/tests/coverage.rs](escrow/src/tests/coverage.rs) | Error code coverage tests |
| [docs/escrow-attestations.md](docs/escrow-attestations.md) | Attestation system design |
| [docs/escrow-security-checklist.md](docs/escrow-security-checklist.md) | Invariant I-5 definition |
| [docs/escrow-data-model.md](docs/escrow-data-model.md) | Data model reference |
| [docs/arch/storage-reference.md](docs/arch/storage-reference.md) | Storage semantics |
| [docs/escrow-error-messages.md](docs/escrow-error-messages.md) | Error documentation |

---

**Document Version:** 1.0  
**Last Updated:** 2026-08-27  
**Status:** Ready for Assignment  
