# ADR-008: Escrow Health Warning System

**Status:** Accepted  
**Date:** 2026-07-28  
**Issue:** #231  
**Refs:** `escrow/src/lib.rs` — `EscrowHealthWarning`, `compute_and_emit_health_warning`, `check_escrow_health`

---

## Context

Off-chain indexers and integrators need real-time visibility into escrow risk states. Examples include:

- **Low funding** + **close to maturity**: funding target may not be met before settlement deadline.
- **Past maturity** but **unfunded**: escrow entered a legally ambiguous state.
- **Funding stalled**: no deposits received for weeks despite available capacity.

The contract currently has no mechanism to signal these conditions. Without such signals, risk teams discover problems reactively (post-maturity) rather than proactively.

---

## Decision

### 1. New Event Type: `EscrowHealthWarning`

Define a new **non-blocking metadata event** emitted when escrow enters a risk state:

```rust
#[contractevent]
pub struct EscrowHealthWarning {
    #[topic]
    pub name: Symbol,                    // "hlth_wrn"
    #[topic]
    pub invoice_id: Symbol,
    pub warning_type: u32,               // Code 4001–4004
    pub funded_amount: i128,
    pub funding_target: i128,
    pub funded_ratio_bps: i64,           // Basis points
    pub time_to_maturity_secs: i64,      // May be negative
    pub recorded_at_ledger_timestamp: u64,
}
```

### 2. Warning Type Codes

| Code | Condition | Emitted When |
|------|-----------|--------------|
| 4001 | `LowFundingRatio` | `funded_ratio_bps < 5000` (< 50%) when open or any status |
| 4002 | `CloseToMaturity` | `0 < time_to_maturity_secs < 86400` (< 1 day) with healthy funding |
| 4003 | `OverMaturity` | `time_to_maturity_secs < 0` and `status == 0` (open) and `unfunded` |
| 4004 | `FundingStalled` | No new funding activity for longer than configured `FundingStallThresholdSecs` while escrow is open (`status == 0`) and underfunded (`funded_amount < funding_target`) |
| 0 | No warning | Default / no risk condition detected |

### 3. Health Computation Logic

**Funded ratio (bps):**
```
funded_ratio_bps = (funded_amount / funding_target) * 10_000
```
Clamped to `i64::MAX` on overflow; returns `10_000` if `funding_target == 0`.

**Time to maturity (seconds):**
```
time_to_maturity_secs = maturity - now
```
Returns `i64::MAX` if `maturity == 0` (no constraint); negative if past maturity.

**Funding staleness (seconds):**
```
if FundingStallThresholdSecs is configured and > 0:
    if LastFundLedgerTimestamp exists:
        time_since_last_fund = now - LastFundLedgerTimestamp
    else:
        time_since_last_fund = now - CreatedAt
    is_stalled = time_since_last_fund > FundingStallThresholdSecs
else:
    is_stalled = false
```

**Determination (priority order):**
- If `is_stalled` AND `status == 0` AND `funded_amount < funding_target` → **4004** (FundingStalled).
- Else if `time_to_maturity_secs < 0` AND `status == 0` AND `funded_amount < funding_target` → **4003** (OverMaturity).
- Else if `0 <= time_to_maturity_secs < 86400` (1 day):
  - If `funded_ratio_bps < 5000` → **4001** (LowFundingRatio).
  - Else → **4002** (CloseToMaturity).
- Else if `funded_ratio_bps < 5000` AND `status == 0` → **4001** (LowFundingRatio, open, no immediate time pressure).
- Else → **0** (No warning).

### 4. Emission Points

Health warnings are emitted at three key transitions:

1. **`fund_impl()`** – After `EscrowFunded` event, check health of the updated escrow.
2. **`settle()`** – After `EscrowSettled` event, check health for audit trail.
3. **`claim_investor_payout()`** – After `InvestorPayoutClaimed` event, check health.

Emission is **non-blocking**: if any condition is met, emit the event; otherwise, emit nothing (code 0 is silent).

### 5. Public Read-Only Endpoint

Provide `check_escrow_health() -> (u32, i64, i64)` for off-chain polling:

```rust
pub fn check_escrow_health(env: Env) -> (u32, i64, i64) {
    // Returns (warning_type, funded_ratio_bps, time_to_maturity_secs)
    // No auth required; pure read operation.
}
```

### 6. Storage & Backward Compatibility

**New Persistent Keys:**
- `DataKey::FundingStallThresholdSecs`: optional configurable staleness threshold (seconds), set at init; absent ⇒ no stall detection.
- `DataKey::LastFundLedgerTimestamp`: updated on every successful `fund` / `fund_with_commitment` call; absent ⇒ never funded.

**Existing Storage:**
- No modification to existing persistent keys.
- `DataKey::CreatedAt` is used to compute staleness when no funding has occurred.

**Backward Compatibility:**
- **Additive keys**: warnings are events only; no schema version bump required.
- **Additive event type**: existing contract instances can upgrade without redeploy.
- **Non-blocking guarantee**: warnings never prevent valid escrow operations.
- Old instances will not have `FundingStallThresholdSecs` set; they will not emit code 4004 warnings unless explicitly upgraded and reinitialized with the threshold.

---

## Rationale

### Why events, not storage?

- **Storage bloat:** every escrow health check would mutate state, consuming ledger quota.
- **Audit trail:** events are immutable and indexed off-chain; more queryable than stored snapshots.
- **Decoupling:** risk logic is decoupled from state transitions; warnings can be disabled or tuned without code changes.

### Why non-blocking?

- A warning is a signal, not a gate. An underfunded escrow is still valid; the escrow may recover with more funding.
- Blocking on warnings risks stranding funds if thresholds are misconfigured.
- Risk teams take action outside the contract (e.g., notify SME, extend maturity).

### Why those thresholds?

- **50% funding ratio**: industry standard for "materially underfunded" (inverse of 50/50 split).
- **1 day to maturity**: sufficient time for most operational responses (notify, inject funds, request extension).
- **Maturity already passed**: legal/financial ambiguity; settled/funded status must be clarified urgently.

---

## Consequences

### Immediate

- Off-chain indexers gain real-time visibility into escrow risk states.
- Risk teams can react proactively (alert SME, initiate recovery).
- Audit trail includes health signals at each state transition.

### Future Enhancements

- **Per-investor health**: warn when an investor's commitment lock expires soon.
- **Admin-configurable thresholds**: allow admin to update or remove stall threshold post-init (out of scope for #232).
- **Scheduled health checks**: emit warnings at fixed intervals (e.g., weekly) to catch escrows approaching staleness.
- **Integration with legal hold**: auto-trigger legal hold if OverMaturity threshold crossed.
- **Multi-stage escalation**: warn at 50% threshold, 75%, etc. before stalling entirely.

---

## Compatibility

### Existing Instances

- Upgrade without redeploy: the new `EscrowHealthWarning` event type is additive.
- Old instances continue operating; indexers will see warnings on the new event stream only after upgrade.

### New Instances

- Deployed with health warnings enabled by default.
- Indexers must consume the new event type to surface risk alerts.

---

## Testing

### Unit Tests

- `test_health_warning_low_funding_ratio`: verify 4001 emission.
- `test_health_warning_close_to_maturity`: verify 4002 emission.
- `test_health_warning_low_funding_close_to_maturity`: verify 4001 takes priority.
- `test_health_warning_over_maturity_unfunded`: verify 4003 emission.
- `test_health_warning_funding_stalled_after_threshold`: verify 4004 emission when stall duration exceeded.
- `test_no_health_warning_funding_not_stalled`: verify no 4004 before threshold.
- `test_no_warning_stall_threshold_not_configured`: verify no 4004 when threshold not set.
- `test_no_warning_funding_stalled_but_fully_funded`: verify no 4004 when escrow is funded (status != 0).
- `test_funding_stalled_never_funded_escrow`: verify 4004 for never-funded escrows after threshold.
- `test_no_health_warning_healthy_escrow`: verify code 0 when healthy.
- `test_no_health_warning_no_maturity_constraint`: verify no time-based warnings when maturity == 0.
- `test_no_health_warning_settled_escrow`: verify settled escrows emit no warnings.
- `test_health_warning_emitted_during_fund`: verify event is published.

### Integration Tests

- Verify health warnings are emitted alongside existing state-change events.
- Verify multiple warnings do not break transaction atomicity.
- Verify `check_escrow_health()` returns correct metrics without auth.
- Verify stall detection correctly compares against both `LastFundLedgerTimestamp` and `CreatedAt`.

### Fuzz Tests

- Random state transitions + maturity advances; verify warning type is always in range [0, 4004].
- Extreme values (i128::MIN / MAX funded amounts) do not panic or overflow.
- Random stall thresholds with various funding timelines.

---

## References

- [ADR-001: State Model](ADR-001-state-model.md)
- [ADR-002: Auth Boundaries](ADR-002-auth-boundaries.md)
- [Escrow Error Messages](../escrow-error-messages.md)
- [Issue #231](https://github.com/karis-ky/escrow-contracts/issues/231)
