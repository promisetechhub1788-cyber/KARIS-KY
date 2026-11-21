# State Inconsistency Detection Entrypoint

## Overview

Added a **read-only diagnostic entrypoint** to detect and report state inconsistencies in the escrow contract. This feature enables auditing, monitoring, and debugging by identifying logical invariant violations without modifying state.

## Components Added

### 1. StateInconsistencyReport Struct

A comprehensive report struct with 10 boolean flags indicating detected inconsistencies:

```rust
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct StateInconsistencyReport {
    pub funded_exceeds_target_not_advanced: bool,
    pub funded_amount_positive_status_open: bool,
    pub zero_funded_amount_advanced_status: bool,
    pub funders_exist_status_open: bool,
    pub no_funders_advanced_status: bool,
    pub snapshot_exists_not_funded: bool,
    pub snapshot_missing_post_funded: bool,
    pub settled_before_maturity_lock: bool,
    pub invalid_funding_amounts: bool,
    pub invalid_status_value: bool,
}
```

**Detected Inconsistencies:**

| Flag | Condition | Meaning |
|------|-----------|---------|
| `funded_exceeds_target_not_advanced` | `funded_amount > funding_target` AND `status < 1` | Funding exceeded target but escrow wasn't advanced to funded status |
| `funded_amount_positive_status_open` | `funded_amount > 0` AND `status == 0` | Principal received but escrow still in open state |
| `zero_funded_amount_advanced_status` | `funded_amount == 0` AND `status >= 1` | Advanced escrow has zero principal |
| `funders_exist_status_open` | `unique_funder_count > 0` AND `status == 0` | Investors funded but escrow remains open |
| `no_funders_advanced_status` | `unique_funder_count == 0` AND `status >= 1` | Advanced escrow has no recorded investors |
| `snapshot_exists_not_funded` | Snapshot set AND `status < 1` | Funding snapshot exists before escrow became funded |
| `snapshot_missing_post_funded` | No snapshot AND `status >= 2` | Advanced escrow missing funding snapshot |
| `settled_before_maturity_lock` | `status == 2` AND `maturity > 0` AND `ledger_time < maturity` | Settlement before maturity lock expired |
| `invalid_funding_amounts` | `funding_target <= 0` OR `amount <= 0` | Invalid funding target or invoice amount |
| `invalid_status_value` | `status > 4` | Status outside valid range (0..=4) |

### 2. StateInconsistenciesDetected Event

```rust
#[contractevent]
pub struct StateInconsistenciesDetected {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub report: StateInconsistencyReport,
}
```

**Published when:** Any inconsistency is detected (i.e., any report flag is `true`). Used for auditing, monitoring, and off-chain alerting.

### 3. detect_state_inconsistencies Entrypoint

A **read-only diagnostic function** that performs comprehensive state validation:

```rust
pub fn detect_state_inconsistencies(env: Env) -> StateInconsistencyReport
```

**Characteristics:**
- **Authorization:** None required — pure read, safe to call at any time
- **Storage mutations:** None — fully read-only
- **Performance:** O(1) — all checks are local, no iteration
- **Event emission:** Only if inconsistencies detected (StateInconsistenciesDetected)
- **Return value:** StateInconsistencyReport with all flags populated

**Usage:**
```rust
let report = LiquifactEscrow::detect_state_inconsistencies(env);
if report.funded_exceeds_target_not_advanced {
    // Handle inconsistency
}
```

## Remediation Runbook

When `detect_state_inconsistencies` reports one or more `true` flags (or a `StateInconsistenciesDetected` event fires), use the table below to interpret each finding and take the corresponding operator action. Every finding type has a remediation entry.

| Finding Flag | What It Means | Operator Remediation |
|--------------|---------------|----------------------|
| `funded_exceeds_target_not_advanced` | Principal received exceeds the funding target, but the escrow status was never advanced to funded (`status < 1`). Funding is effectively complete but the lifecycle is stuck. | Do **not** mutate state manually. Capture the report and event, then escalate to the contract maintainers to advance the escrow via the normal funding-close path. Pause any downstream settlement until status is corrected. |
| `funded_amount_positive_status_open` | `funded_amount > 0` while `status == 0` (open). Funds were recorded but the escrow still appears open. | Verify the funding transaction(s) on-chain and confirm the expected status transition. If the transition was missed, escalate to maintainers to reconcile status; do not accept new contributions until resolved. |
| `zero_funded_amount_advanced_status` | Escrow advanced past open (`status >= 1`) but `funded_amount == 0`. An advanced escrow should always carry principal. | Treat as a critical data-integrity issue. Freeze further operations on this escrow, preserve the report/event evidence, and escalate to maintainers to investigate the funding record and status history. |
| `funders_exist_status_open` | `unique_funder_count > 0` while `status == 0`. Investors are recorded but the escrow remains open. | Confirm whether the funding-close transition was expected. Escalate to maintainers to advance status; notify affected investors that the escrow is under reconciliation. |
| `no_funders_advanced_status` | Escrow advanced (`status >= 1`) but `unique_funder_count == 0`. No investors recorded despite advancement. | Treat as a critical integrity issue. Halt settlement/withdrawal flows for this escrow, retain evidence, and escalate to maintainers to audit the funder registry and advancement path. |
| `snapshot_exists_not_funded` | A funding-close snapshot is set while `status < 1`. The snapshot should only exist once the escrow is funded. | Do not rely on the snapshot for calculations. Escalate to maintainers to reconcile the snapshot with the actual status; verify no downstream logic consumed the premature snapshot. |
| `snapshot_missing_post_funded` | No funding-close snapshot exists while `status >= 2`. Advanced escrows require a snapshot for correct accounting. | Block any operation that depends on the snapshot (e.g. yield/settlement math). Escalate to maintainers to reconstruct or restore the snapshot before proceeding. |
| `settled_before_maturity_lock` | `status == 2` (settled) with `maturity > 0` and `ledger_time < maturity`. Settlement occurred before the maturity lock expired. | Treat as a policy violation. Preserve the report/event and ledger timestamps, notify compliance, and escalate to maintainers to review the settlement path. Do not process further payouts on this escrow. |
| `invalid_funding_amounts` | `funding_target <= 0` or invoice `amount <= 0`. The escrow was configured with non-positive amounts. | Flag the escrow as misconfigured. Prevent new contributions and escalate to maintainers to correct the configuration; verify no funds were accepted against the invalid target. |
| `invalid_status_value` | `status > 4`, outside the valid range `0..=4`. The stored status is corrupt or from an incompatible version. | Treat as a critical integrity issue. Halt all operations on the escrow, capture the raw status value and report, and escalate to maintainers for recovery. Do not attempt to coerce the status manually. |

**General operator guidance:**
- `detect_state_inconsistencies` is read-only and safe to call repeatedly; use it as the first step in any incident triage.
- Always capture the full `StateInconsistencyReport` and the `StateInconsistenciesDetected` event (including `invoice_id`) as evidence before escalating.
- Never mutate contract state directly to "fix" a finding — remediation is performed by the contract maintainers through the supported entrypoints.
- For any critical finding (`zero_funded_amount_advanced_status`, `no_funders_advanced_status`, `invalid_status_value`), pause dependent operations immediately and escalate.

## Test Coverage

Comprehensive test suite in `escrow/src/tests/state_inconsistency.rs` with 8 tests:

1. **test_valid_escrow_no_inconsistencies** — Validates all flags false on valid initialized escrow
2. **test_inconsistency_funded_exceeds_target_not_advanced** — Tests detection when funded > target but status < 1
3. **test_inconsistency_funded_amount_positive_status_open** — Tests detection when funded > 0 but status == 0
4. **test_inconsistency_funders_exist_status_open** — Tests detection when funders > 0 but status == 0
5. **test_multiple_inconsistencies_detected** — Validates multiple flags can be true simultaneously
6. **test_all_inconsistency_flags_false_on_valid_state** — Comprehensive validation of all flags on valid state
7. **test_inconsistency_detection_is_readonly** — Verifies repeated calls return identical reports (no state change)
8. **test_funded_exceeds_with_exact_match** — Validates exact target match doesn't trigger inconsistency

## Integration Points

### Contract Interface Version

No change to `CONTRACT_INTERFACE_VERSION` — the new entrypoint is added without modifying existing signatures or return types.

### Storage Keys

Uses existing `DataKey` variants:
- `DataKey::Escrow` (read)
- `DataKey::FundingCloseSnapshot` (read)
- `DataKey::UniqueFunderCount` (read)

### Dependencies

None added — uses only existing Soroban SDK and contract types.

## Use Cases

1. **Off-chain auditing:** Periodically call to detect anomalies
2. **Real-time monitoring:** Watch for StateInconsistenciesDetected events
3. **Post-incident debugging:** Call after error or unexpected behavior
4. **Integration validation:** Verify escrow state before sensitive operations
5. **Compliance checks:** Document state conformance for regulatory audits

## Design Decisions

1. **Read-only by design:** Diagnostic entrypoints should never mutate state
2. **No auth required:** Auditing should be accessible to anyone
3. **All-or-nothing reporting:** Return complete report in one call (no iterator)
4. **Event-only on issues:** Reduce blockchain bloat for healthy escrows
5. **Immutable report:** StateInconsistencyReport derives Clone for event payload
6. **Fast path:** All O(1) lookups, no iteration over investor lists

## Future Extensions

Potential enhancements (not implemented):
- Per-investor balance reconciliation
- Yield computation validation
- Attestation log integrity checks
- Cross-escrow consistency checks (if registry exists)
- Timeline violation detection (e.g., maturity past settlement)

## Files Modified

1. `/workspaces/KARIS-KY/escrow/src/lib.rs`
   - Added `StateInconsistencyReport` struct (line 674)
   - Added `StateInconsistenciesDetected` event (line 999)
   - Added `detect_state_inconsistencies` entrypoint (line 1978)

2. `/workspaces/KARIS-KY/escrow/src/tests.rs`
   - Added `mod state_inconsistency` declaration

3. `/workspaces/KARIS-KY/escrow/src/tests/state_inconsistency.rs`
   - New comprehensive test module with 8 tests

## Compatibility

- ✅ Schema version unchanged (still 6)
- ✅ Contract interface version unchanged
- ✅ No breaking changes to existing entrypoints
- ✅ No new storage keys required
- ✅ Backward compatible with all prior deployments
