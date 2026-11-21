# FEAT-011: Escrow Health Check Entrypoint

**Issue ID:** FEAT-011

**Title:** `feat: add escrow health check entrypoint returning structured warnings`

**Status:** Fully Specified

---

## 1. Executive Summary

Implement a public, read-only contract entrypoint `check_escrow_health()` that proactively detects and reports escrow risk states (underfunding, imminent maturity, past maturity, etc.) via typed warning codes. This enables off-chain risk systems to monitor escrow health in real time and trigger alerts or mitigation actions before settlement failures occur.

---

## 2. Problem Statement

### Current Situation
- Escrow state is queryable via `get_escrow()`, but risk signals are implicit in the data.
- Integrators must write off-chain logic to compute health metrics (funding ratio, time to maturity, etc.).
- Risk conditions (e.g., underfunded + close to maturity) are discovered reactively, not proactively.
- No standardized warning mechanism for cross-system integration.

### Desired State
- Contract emits structured, typed health warnings at key state transitions.
- Risk teams can subscribe to warning events via standard indexer APIs.
- Standardized warning codes enable consistent risk classification across systems.
- Read-only entrypoint allows real-time health polling without auth.

---

## 3. Requirements

### 3.1 Functional Requirements

| Requirement | Status | Notes |
|-------------|--------|-------|
| Define four warning type codes (4001–4004) | ✓ Implemented | Codes defined in ADR-008 |
| Compute funded ratio in basis points | ✓ Implemented | `(funded / target) * 10_000` |
| Compute time to maturity in seconds | ✓ Implemented | `maturity - now`; returns `i64::MAX` if no constraint |
| Emit warning event at fund, settle, claim transitions | ✓ Implemented | Hooked into `fund_impl()`, `settle()`, `claim_investor_payout()` |
| Public read-only entrypoint `check_escrow_health()` | ✓ Implemented | Returns `(u32, i64, i64)` |
| Pretty-print formatted summary via new `get_escrow_health()` | ⚠ Partial | Need formatted variant |

### 3.2 Non-Functional Requirements

| Requirement | Status | Notes |
|-------------|--------|-------|
| No state mutation | ✓ | Pure read operation |
| No authorization required | ✓ | Public, read-only |
| Gas cost < 500k | ✓ | Single storage read + arithmetic |
| Backward compatible | ✓ | Additive event; no schema bump |
| Deterministic | ✓ | No external oracle dependency |

---

## 4. Warning Type Codes

### Code 4001: Low Funding Ratio

**Condition:** Escrow is underfunded (< 50% of target) at any time during open/funded states.

**Triggers:**
- Escrow status = 0 (open) or 1 (funded) AND
- `(funded_amount / funding_target) * 10_000 < 5000` (< 50%)

**Implications:**
- Funding target may not be met before settlement.
- At settlement, shortfall could trigger investor refund or partial settlement depending on policy.

**Example:**
- Target: 100M, Funded: 40M → Warning 4001 ✓
- Target: 100M, Funded: 50M → No warning (borderline) ✗
- Target: 100M, Funded: 100M → No warning ✗

---

### Code 4002: Close to Maturity

**Condition:** Time to maturity is between 0 and 1 day (86400 seconds), and funding is healthy (≥ 50%).

**Triggers:**
- Escrow status = any AND
- `0 ≤ (maturity - now) < 86400` AND
- `funded_ratio_bps ≥ 5000` (well-funded)

**Implications:**
- Limited time to respond to operational issues (e.g., SME withdrawal failure, indexing delays).
- Settlement window is imminent; on-chain and off-chain systems should be online and synced.

**Example:**
- Maturity: in 12 hours, Funded: 100M / 100M → Warning 4002 ✓
- Maturity: in 2 days, Funded: 100M / 100M → No warning ✗
- Maturity: in 12 hours, Funded: 40M / 100M → Warning 4001, not 4002 ✗

---

### Code 4003: Over Maturity (Ambiguous State)

**Condition:** Maturity deadline has passed, escrow is still open (unfunded), and funding is incomplete.

**Triggers:**
- Escrow status = 0 (open) AND
- `(maturity - now) < 0` (past maturity) AND
- `funded_amount < funding_target` (underfunded)

**Implications:**
- **Critical state**: Escrow has entered a legally and operationally ambiguous zone.
- Settlement may be blocked or require governance intervention.
- Investor claims may be complicated if SME failed to settle before maturity.
- **Action required:** admin must either extend maturity, settle partial, or escalate.

**Example:**
- Maturity: 2 hours ago, Funded: 40M / 100M, Status: 0 → Warning 4003 ✓ (critical)
- Maturity: 2 hours ago, Funded: 100M / 100M, Status: 0 → No warning (will settle normally)
- Maturity: 2 hours ago, Status: 2 (settled) → No warning (already settled)

---

### Code 4004: Funding Stalled (Reserved)

**Status:** Reserved for future use.

**Proposed condition:** No deposits received for an extended period (e.g., > 7 days) while escrow remains open and underfunded.

**Implementation:** Requires tracking last-deposit timestamp; deferred to v2.

---

### Code 0: No Warning

Emitted when escrow is in a healthy state (or no risk condition is detected):
- Well-funded, no time pressure.
- Recently settled, no further risk.
- Cancelled or otherwise terminal state.

---

## 5. Event Emission Strategy

### 5.1 Emission Points

Warnings are emitted **after** successful state transitions at these three points:

1. **`fund_impl()` (after EscrowFunded)**
   - Checks health after investor deposit is recorded.
   - Flags if funding is below target and deadline is near.

2. **`settle()` (after EscrowSettled)**
   - Checks health after settlement.
   - Flags if escrow is being settled in an ambiguous state (past maturity + underfunded).

3. **`claim_investor_payout()` (after InvestorPayoutClaimed)**
   - Checks health after investor claims payout.
   - Flags if future claims are at risk due to remaining escrow state.

### 5.2 Emission Logic

```rust
fn compute_and_emit_health_warning(env: &Env, invoice_id: Symbol) {
    let (warning_type, funded_ratio_bps, time_to_maturity_secs) = 
        Self::check_escrow_health(env);
    
    if warning_type > 0 {  // Only emit if warning detected (non-zero code)
        let escrow = Self::get_escrow(env);
        env.events().publish(
            (Symbol::new(env, "hlth_wrn"), invoice_id),
            EscrowHealthWarning {
                name: Symbol::new(env, "hlth_wrn"),
                invoice_id,
                warning_type,
                funded_amount: escrow.funded_amount,
                funding_target: escrow.funding_target,
                funded_ratio_bps,
                time_to_maturity_secs,
                recorded_at_ledger_timestamp: env.ledger().timestamp(),
            },
        );
    }
}
```

### 5.3 Non-Blocking Guarantee

- Warnings are **emitted, not enforced**.
- A warning does not prevent escrow operations; it is a signal only.
- If warning computation fails (e.g., overflow), emit silently; do not block the operation.

---

## 6. Public API

### 6.1 Low-Level Endpoint: `check_escrow_health()`

**Signature:**
```rust
pub fn check_escrow_health(env: Env) -> (u32, i64, i64)
```

**Returns:**
- `(warning_type: u32, funded_ratio_bps: i64, time_to_maturity_secs: i64)`
- `warning_type`: Code 0–4004 indicating the primary risk condition.
- `funded_ratio_bps`: Funded ratio in basis points (0–10_000+), clamped to `i64::MAX` on overflow.
- `time_to_maturity_secs`: Seconds until maturity (may be negative if past maturity); `i64::MAX` if no maturity constraint.

**Authorization:** None (public read).

**Gas:** ~100k (single storage read + arithmetic).

**Example:**
```
Input: Escrow with 40M / 100M funded, 12 hours to maturity
Output: (4001, 4000, 43200)
         (low funding ratio, 40%, 43200 secs = 12 hours)
```

### 6.2 High-Level Endpoint: `get_escrow_health()` (New)

**Signature:**
```rust
pub fn get_escrow_health(env: Env) -> EscrowHealth
```

**Returns:**
```rust
pub struct EscrowHealth {
    pub warning_type: u32,
    pub warning_label: String,  // "low_funding", "close_to_maturity", etc.
    pub funded_ratio_bps: i64,
    pub funded_ratio_percent: f64,  // For readability
    pub time_to_maturity_secs: i64,
    pub time_to_maturity_days: f64,  // For readability
    pub is_healthy: bool,  // warning_type == 0
    pub recommendation: String,  // Suggested action
    pub recorded_at_ledger_timestamp: u64,
}
```

**Authorization:** None (public read).

**Gas:** ~150k (computes ratio + formats strings).

**Purpose:** Developer-friendly alternative to `check_escrow_health()` for human consumption.

---

## 7. Implementation Notes

### 7.1 Ratios & Arithmetic

**Funded ratio in basis points:**
```rust
let funded_ratio_bps: i64 = if escrow.funding_target > 0 {
    let numerator = (escrow.funded_amount as i128).saturating_mul(10_000);
    let ratio = numerator / (escrow.funding_target as i128);
    if ratio > i64::MAX as i128 {
        i64::MAX
    } else if ratio < 0 {
        0
    } else {
        ratio as i64
    }
} else {
    10_000  // Assume 100% if target is 0 (edge case)
};
```

**Time to maturity in seconds:**
```rust
let time_to_maturity_secs: i64 = if escrow.maturity > 0 {
    let maturity_i64 = escrow.maturity as i64;
    let now_i64 = env.ledger().timestamp() as i64;
    maturity_i64.saturating_sub(now_i64)
} else {
    i64::MAX  // No constraint
};
```

### 7.2 Warning Determination Order

The following priority order ensures exactly one warning is emitted:

1. **OverMaturity (4003):** If `time_to_maturity < 0` AND `status == 0` AND `underfunded` → 4003.
2. **CloseToMaturity (4002):** If `0 ≤ time_to_maturity < 86400` AND `funded ≥ 50%` → 4002.
3. **LowFundingRatio (4001):** If `0 ≤ time_to_maturity < 86400` AND `funded < 50%` → 4001.
4. **LowFundingRatio (4001):** If `time_to_maturity ≥ 86400` AND `status == 0` AND `funded < 50%` → 4001.
5. **NoWarning (0):** Otherwise → 0.

### 7.3 Storage & Backward Compatibility

- **No new persistent storage keys:** warnings are events only.
- **Additive event type:** `EscrowHealthWarning` is a new event; existing contracts can emit it after upgrade.
- **No schema version bump:** `SCHEMA_VERSION` remains unchanged.

---

## 8. Testing Strategy

### 8.1 Unit Tests

| Test Case | Setup | Expected Output |
|-----------|-------|-----------------|
| `test_health_low_funding_ratio` | 40% funded, 5 days to maturity | Code 4001 |
| `test_health_close_to_maturity` | 100% funded, 12 hours to maturity | Code 4002 |
| `test_health_over_maturity` | 40% funded, 2 hours past maturity | Code 4003 |
| `test_health_no_warning` | 100% funded, 10 days to maturity | Code 0 |
| `test_health_zero_target` | 0 funded / 0 target | Code 0 (edge case) |
| `test_health_no_maturity` | Any funding, no maturity set | Code 0 or 4001 based on ratio |

### 8.2 Integration Tests

- Fund → verify health warning emitted and logged.
- Settle → verify health warning emitted at settlement.
- Claim → verify health warning emitted after claim.
- Verify warnings are **non-blocking** (operations succeed despite warnings).

### 8.3 Event Tests

- Subscribe to `hlth_wrn` event topic.
- Verify event payload matches return values from `check_escrow_health()`.
- Verify event timestamp matches ledger timestamp.

---

## 9. Acceptance Criteria

- [ ] `check_escrow_health()` public endpoint exists and returns `(u32, i64, i64)`.
- [ ] `get_escrow_health()` public endpoint exists and returns formatted `EscrowHealth`.
- [ ] Warning codes 4001–4003 are correctly computed and deterministic.
- [ ] Warnings are emitted (non-blocking) at `fund_impl()`, `settle()`, `claim_investor_payout()`.
- [ ] All unit and integration tests pass.
- [ ] No new persistent storage keys added.
- [ ] Schema version unchanged.
- [ ] Documentation added to `docs/escrow-health-check.md`.
- [ ] README updated with health check usage example.

---

## 10. Security & Risk Mitigations

| Risk | Mitigation |
|------|-----------|
| DoS via repeated health checks | No state mutation; gas cost is bounded (< 150k). |
| Integer overflow in ratio calc | Use `saturating_mul` and clamp to `i64::MAX`. |
| Time-travel attacks (maturity check) | Dependency on `env.ledger().timestamp()`; Soroban validates. |
| Silent warning failures | Emit warnings in a separate phase; do not block core operations. |

---

## 11. Future Enhancements

1. **Configurable thresholds:** Admin may adjust warning ratios per escrow instance.
2. **Per-investor health:** Warn when an investor's commitment lock expires soon.
3. **Scheduled health checks:** Emit warnings at fixed intervals (e.g., weekly) to catch stalled funding.
4. **Integration with legal hold:** Auto-trigger legal hold if OverMaturity threshold crossed.
5. **Code 4004 (FundingStalled):** Implement once `last_deposit_timestamp` is tracked.

---

## 12. References

- [ADR-008: Escrow Health Warning System](docs/adr/ADR-008-escrow-health-warnings.md)
- [FEATURE_220_REPL_DESIGN.md](FEATURE_220_REPL_DESIGN.md)
- Related: FEAT-010 (REPL), FEAT-011 (Health Check)
