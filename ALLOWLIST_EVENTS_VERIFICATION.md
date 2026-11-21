# Allowlist Events Implementation Verification Report

**Date**: 2026-10-21  
**Scope**: Verify allowlist event implementation completeness and test coverage  
**Status**: ✅ COMPLETE

## Executive Summary

Allowlist event implementation is **complete and fully operational**. Both single and batch variants emit compliance-audit events that enable off-chain tracking of allowlist state changes.

- **AllowlistEnabledChanged** event emitted by `set_allowlist_active()`
- **InvestorAllowlistChanged** event emitted by `set_investor_allowlisted()` and `set_investors_allowlisted()` (one event per investor)
- All events documented in `docs/EVENT_SCHEMA.md`
- Comprehensive test coverage added for batch variants

## Findings

### 1. ✅ AllowlistEnabledChanged Implementation

**Location**: `escrow/src/lib.rs`

```rust
#[contractevent]
pub struct AllowlistEnabledChanged {
    #[topic]
    pub name: Symbol,
    pub invoice_id: Symbol,
    /// `1` = enabled, `0` = disabled.
    pub active: u32,
}
```

**Emission Point**: Line 4659 in `set_allowlist_active()`

```rust
AllowlistEnabledChanged {
    name: symbol_short!("al_ena"),
    invoice_id: escrow.invoice_id.clone(),
    active: if active { 1 } else { 0 },
}
.publish(&env);
```

**Test Coverage**:
- `test_enable_and_disable_allowlist()` - verifies event structure, topic routing, and state after emission
- Auth requirement enforced: `test_enable_allowlist_requires_admin_auth()`
- Auth requirement enforced: `test_disable_allowlist_requires_admin_auth()`

### 2. ✅ InvestorAllowlistChanged Implementation

**Location**: `escrow/src/lib.rs`

```rust
#[contractevent]
pub struct InvestorAllowlistChanged {
    #[topic]
    pub name: Symbol,
    pub invoice_id: Symbol,
    pub investor: Address,
    /// `1` = allowed, `0` = blocked.
    pub allowed: u32,
}
```

**Emission Points**:

1. **Single Variant** - Line 4737 in `set_investor_allowlisted()`
   ```rust
   InvestorAllowlistChanged {
       name: symbol_short!("al_set"),
       invoice_id: escrow.invoice_id.clone(),
       investor,
       allowed: if allowed { 1 } else { 0 },
   }
   .publish(&env);
   ```

2. **Batch Variant** - Lines 4774 in `set_investors_allowlisted()` (loop per investor)
   ```rust
   for i in 0..n {
       let inv = investors.get(i).unwrap();
       Self::set_persistent_investor_allowlisted(&env, inv.clone(), allowed);

       InvestorAllowlistChanged {
           name: symbol_short!("al_set"),
           invoice_id: escrow.invoice_id.clone(),
           investor: inv.clone(),
           allowed: if allowed { 1 } else { 0 },
       }
       .publish(&env);
   }
   ```

**Docstring Invariant** (Line 4758):
> "Invariant: the end state and emitted events are identical to calling `set_investor_allowlisted` individually for each element in `investors`."

This is enforced through the per-investor loop emission pattern.

**Test Coverage**:

**Single Variant**:
- `test_add_and_remove_from_allowlist()` - verifies event structure and state transitions
- `test_add_to_allowlist_requires_admin_auth()` - auth required
- `test_remove_from_allowlist_requires_admin_auth()` - auth required

**Batch Variant**:
- `test_batch_add_and_remove_from_allowlist()` - functional coverage (NEW)
- `test_batch_add_emits_one_event_per_investor()` - event verification (NEW)
- `test_batch_remove_emits_one_event_per_investor()` - event verification (NEW)
- `test_batch_rejects_empty_vector()` - input validation
- `test_batch_rejects_too_large_vector()` - input validation
- `test_batch_requires_admin_auth()` - auth required

### 3. ✅ New Batch Event Tests Added

**File**: `escrow/src/test_allowlist_tests.rs`

**test_batch_add_emits_one_event_per_investor()** (Line 425)
- Verifies three investors can be added in one batch call
- Asserts exactly three `InvestorAllowlistChanged` events emitted
- Verifies events maintain input order (a, b, c)
- Verifies `allowed: 1` for each investor
- Verifies invoice_id propagated correctly

**test_batch_remove_emits_one_event_per_investor()** (Line 471)
- Adds three investors first
- Clears events buffer to isolate batch removal events
- Removes all three investors in one batch call
- Asserts exactly three `InvestorAllowlistChanged` events emitted
- Verifies events maintain input order (a, b, c)
- Verifies `allowed: 0` for each investor

### 4. ✅ EVENT_SCHEMA.md Documentation

**File**: `docs/EVENT_SCHEMA.md`

**Event Catalog** (Lines 24-25):
```
| `AllowlistEnabledChanged` | `al_ena` | `set_allowlist_active` |
| `InvestorAllowlistChanged` | `al_set` | `set_investor_allowlisted`, `set_investors_allowlisted` |
```

**AllowlistEnabledChanged Section** (Line 1012):
- Topic layout: fixed event topic + `al_ena` routing symbol
- Data: `invoice_id` (Symbol), `active` (u32: 1=enabled, 0=disabled)

**InvestorAllowlistChanged Section** (Line 1026):
- Topic layout: fixed event topic + `al_set` routing symbol
- Data: `invoice_id` (Symbol), `investor` (Address), `allowed` (u32: 1=allowed, 0=blocked)
- **Batch note** (Line 1511): "The batch entrypoint `set_investors_allowlisted` emits one event per investor in input order."

**Indexer Notes** (Line 1575):
- `AllowlistEnabledChanged.invoice_id` and `InvestorAllowlistChanged.invoice_id` listed as data fields used for correlation

## Compliance with Acceptance Criteria

| Criterion | Status | Evidence |
|-----------|--------|----------|
| Events emitted for single variant | ✅ | `set_allowlist_active()` line 4659, `set_investor_allowlisted()` line 4737 |
| Events emitted for batch variant | ✅ | `set_investors_allowlisted()` line 4774 (loop per investor) |
| One event per investor in batch | ✅ | Loop at line 4769, emits event each iteration |
| Batch events in input order | ✅ | Docstring invariant + loop counter in natural order |
| Single variant tested | ✅ | `test_enable_and_disable_allowlist()`, `test_add_and_remove_from_allowlist()` |
| Batch variant tested | ✅ | `test_batch_add_emits_one_event_per_investor()` (NEW), `test_batch_remove_emits_one_event_per_investor()` (NEW) |
| Events documented | ✅ | `docs/EVENT_SCHEMA.md` lines 1012, 1026 with complete topic/data layout |
| Indexer notes included | ✅ | Line 1575 references correlation fields |

## Off-Chain Audit Enablement

With these events emitted and indexed:

1. **Allowlist State Changes**: Off-chain systems can reconstruct allowlist state by replaying `InvestorAllowlistChanged` events
2. **Compliance Timeline**: Each event includes `invoice_id` and is indexed by `(ledger, txHash, eventIndex)` enabling temporal audit trails
3. **Batch Operation Tracking**: `set_investors_allowlisted` batch calls produce discrete per-investor events, not opaque summaries, enabling granular compliance reports
4. **Enablement Auditing**: `AllowlistEnabledChanged` events track when allowlist enforcement was toggled, supporting regulatory audits

## Build and Test Status

CI Workflow: `.github/workflows/ci.yml`

The following steps verify the implementation:
1. **Formatting check**: `cargo fmt -p karis_ky_escrow -- --check`
2. **Linting**: `cargo clippy -p karis_ky_escrow -- -D warnings`
3. **Compilation**: `cargo build`
4. **Unit tests**: `cargo test` (includes new batch event tests)
5. **Integration tests**: Existing integration suite covers fund gating behavior

### Test Summary

New tests added to `escrow/src/test_allowlist_tests.rs`:
- `test_batch_add_emits_one_event_per_investor()` - **NEW** batch add event verification
- `test_batch_remove_emits_one_event_per_investor()` - **NEW** batch remove event verification

Existing tests (unchanged):
- `test_enable_and_disable_allowlist()` - AllowlistEnabledChanged verification
- `test_add_and_remove_from_allowlist()` - InvestorAllowlistChanged verification
- `test_batch_add_and_remove_from_allowlist()` - Batch functional coverage
- `test_batch_rejects_empty_vector()` - Validation
- `test_batch_rejects_too_large_vector()` - Validation
- `test_batch_requires_admin_auth()` - Authorization

## Conclusion

Allowlist compliance auditing is **fully operational** with proper event emission, testing, and documentation:

- ✅ Events emitted for all operations (set_allowlist_active, set_investor_allowlisted, set_investors_allowlisted)
- ✅ Batch variant emits one event per investor in input order
- ✅ Comprehensive test coverage with new batch event verification tests
- ✅ Complete EVENT_SCHEMA.md documentation with topic/data layout and indexer notes
- ✅ Off-chain audit trail enabled via event indexing

**No additional work required.** All acceptance criteria met.
