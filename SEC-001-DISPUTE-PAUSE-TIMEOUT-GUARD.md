# [SEC-001] Add timeout guard to emergency_pause to prevent indefinite admin lock

**Category:** Security (SEC)  
**Status:** Needs full specification  
**Severity:** High  
**Priority:** P1  
**Component:** Escrow Contract (`escrow/src/lib.rs`)  

---

## Executive Summary

The `pause_dispute` entrypoint allows the admin to freeze an escrow instance indefinitely by setting an arbitrarily large `duration_secs` value without an upper bound. This creates a denial-of-service (DoS) vector that could lock legitimate dispute resolution indefinitely, either through admin negligence or compromise.

**Current behavior:** A pause duration can be set to any value ≤ `u64::MAX` seconds (~584 billion years).

**Risk:** If an admin account is compromised or mismanaged, an attacker can call `pause_dispute` with an enormous duration, freezing investor operations (`fund`, `settle`, `withdraw`, `claim_investor_payout`) until either a formal contract redeploy occurs or governance takes manual corrective action. This violates the operational resilience assumption that legitimate disputes should be resolvable within a reasonable timeframe.

---

## Full Description

### Background

The escrow contract introduced the dispute pause mechanism (schema version 7) as a temporary state overlay distinct from legal hold. It allows the admin to freeze escrow operations while resolving off-chain invoice disputes (e.g., SME-reported invoice validity challenges, invoice amount discrepancies, or other contract performance disagreements).

**Current entrypoint signature:**

```rust
pub fn pause_dispute(env: Env, ticket_id: String, duration_secs: u64) {
    ensure(&env, !ticket_id.is_empty(), EscrowError::DisputeTicketIdEmpty);
    ensure(&env, duration_secs > 0, EscrowError::DisputePauseDurationNotPositive);
    
    let escrow = Self::load_escrow_require_admin(&env);
    let now = env.ledger().timestamp();
    let expires_at = now
        .checked_add(duration_secs)
        .unwrap_or_else(|| fail(&env, EscrowError::LedgerTimestampOverflow));
    
    // ... store pause state and emit event
}
```

**Current validation:**
- `ticket_id` must be non-empty ✓
- `duration_secs` must be positive ✓
- Overflow check on timestamp addition ✓
- Admin authentication required ✓

**Missing validation:**
- **No maximum duration cap.** A malicious or negligent admin can set `duration_secs = 86400 * 365 * 10_000` (10,000 years) or higher.

### Affected Code Paths

1. **Direct impact:** Any escrow operation that checks `is_dispute_paused()`:
   - `fund()` → [`EscrowError::DisputePausedBlocksFunding`]
   - `settle()` → [`EscrowError::DisputePausedBlocksSettlement`]
   - `withdraw()` → [`EscrowError::DisputePausedBlocksWithdrawal`]
   - `claim_investor_payout()` → [`EscrowError::DisputePausedBlocksInvestorClaims`]

2. **Operational impact:** Recovery from an indefinite pause requires:
   - Admin manual `resume_dispute()` call (vulnerable to same admin compromise).
   - Contract redeploy (high operational cost, not designed for this recovery path).
   - Governance intervention (slow, requires multisig ceremony).

3. **Investor impact:** Investors cannot:
   - Withdraw funded amounts if settlement is blocked by indefinite pause.
   - Claim payouts after settlement if pause extends past settlement time.
   - Refund principal if funding phase is perpetually paused.

### Threat Models

#### 1. Admin key compromise (primary risk)
- Attacker gains access to admin's signing key or hot wallet.
- Attacker calls `pause_dispute("ticket-99999", u64::MAX)` to freeze escrow indefinitely.
- Legitimate dispute resolution is blocked; governance must formally intervene.

#### 2. Admin negligence or misconfiguration
- Operations team accidentally configures a pause duration in seconds that was meant to be minutes, hours, or days.
- Example: `pause_dispute("DISP-0001", 999_999_999)` → ~31.7 years of unintended pause.

#### 3. Escalating dispute locks (chained attacks)
- Attacker pauses the escrow; legitimate admin tries to resume.
- Before the resume can be executed, attacker (or a second compromised signer) pauses again with a longer duration.
- Pause cascading: governance consensus slow, recovery blocked.

---

## Steps to Reproduce / Proposed Solution

### Reproduction Scenario

**Environment:** Soroban testnet or local Soroban environment.

**Setup:**
1. Deploy escrow contract with test admin and SME accounts.
2. Initialize an escrow instance with standard parameters.
3. Fund the escrow to a non-settled state.

**Reproduce indefinite lock:**

```bash
# Call pause_dispute with an extreme duration
stellar contract invoke \
  --network testnet \
  --source <admin-secret> \
  -- --contract <contract-id> \
  -- pause_dispute \
  -- ticket_id "DISP-EXTREME-001" \
  -- duration_secs 99999999999

# Attempt to perform any escrow operation
stellar contract invoke \
  --network testnet \
  --source <investor-secret> \
  -- --contract <contract-id> \
  -- fund \
  -- ... [other params]

# Expected failure: DisputePausedBlocksFunding error
# Actual failure: DisputePausedBlocksFunding error (correct behavior, but pause never expires)
```

**Validation:**
- Read the pause state: `get_dispute_pause()` returns `expires_at` far in the future.
- Call `is_dispute_paused()` → returns `true` for the next 31.7 years.
- No operation can proceed without manual `resume_dispute()` or contract redeploy.

---

### Proposed Solution

Introduce a configurable **maximum dispute pause duration** enforced at the `pause_dispute` entrypoint. This ensures reasonable dispute resolution timeframes and prevents indefinite lock-in.

#### 1. Add `MAX_DISPUTE_PAUSE_DURATION_SECS` constant

```rust
/// Maximum allowed dispute pause duration (14 days in seconds).
/// This guards against indefinite admin locks and ensures disputes can be escalated to governance.
/// Rationale:
/// - 14 days aligns with standard business dispute resolution windows.
/// - Sufficient time for ticket coordination between karis-ky operations and SME counterparty.
/// - Short enough that indefinite pause is clearly a governance red flag, not operational.
const MAX_DISPUTE_PAUSE_DURATION_SECS: u64 = 14 * 24 * 60 * 60; // 1,209,600 seconds
```

**Rationale for 14 days:**
- Aligns with SLA-like dispute windows in invoice finance (typical: 3–15 days).
- Long enough for operational teams to coordinate ticket resolution.
- Short enough that indefinite pause is immediately obvious as a governance escalation.
- Business policy can adjust at contract init or governance proposal; hardcoded constant is conservative baseline.

#### 2. Add validation error code

```rust
pub enum EscrowError {
    // ... existing codes ...
    
    /// `pause_dispute` received a duration exceeding the maximum allowed pause window.
    /// See `MAX_DISPUTE_PAUSE_DURATION_SECS` for the limit.
    DisputePauseDurationExceedsMax = 170,
}
```

#### 3. Add validation to `pause_dispute`

```rust
pub fn pause_dispute(env: Env, ticket_id: String, duration_secs: u64) {
    ensure(
        &env,
        !ticket_id.is_empty(),
        EscrowError::DisputeTicketIdEmpty,
    );
    ensure(
        &env,
        duration_secs > 0,
        EscrowError::DisputePauseDurationNotPositive,
    );
    // NEW: Enforce maximum duration
    ensure(
        &env,
        duration_secs <= MAX_DISPUTE_PAUSE_DURATION_SECS,
        EscrowError::DisputePauseDurationExceedsMax,
    );

    let escrow = Self::load_escrow_require_admin(&env);
    let now = env.ledger().timestamp();
    let expires_at = now
        .checked_add(duration_secs)
        .unwrap_or_else(|| fail(&env, EscrowError::LedgerTimestampOverflow));

    let pause_state = DisputePauseState {
        ticket_id: ticket_id.clone(),
        paused_at_ledger_timestamp: now,
        expires_at_ledger_timestamp: expires_at,
    };

    env.storage()
        .instance()
        .set(&DataKey::DisputePaused, &pause_state);

    DisputePausedEvt {
        name: symbol_short!("disppause"),
        invoice_id: escrow.invoice_id.clone(),
        ticket_id,
        action: 1,
        paused_at: now,
        expires_at,
    }
    .publish(&env);
}
```

#### 4. Documentation & operational guidance

- Update `docs/DEPLOYER_SECURITY.md` §3.1 to monitor for `DisputePausedEvt` with durations approaching the max (e.g., > 7 days) as early warning of potential governance issues.
- Add to `docs/escrow-compliance-guide.md`: operational SLA for resolving tickets within the pause window.
- Update `docs/escrow-error-messages.md` with error code 170.

---

## Expected vs. Actual Behavior

| Behavior | Expected | Actual (before fix) | Actual (after fix) |
|----------|----------|-------------------|-------------------|
| `pause_dispute("DISP-001", 3600)` (1 hour) | Accept, pause for 1 hour | ✓ Accept | ✓ Accept |
| `pause_dispute("DISP-001", 86400)` (1 day) | Accept, pause for 1 day | ✓ Accept | ✓ Accept |
| `pause_dispute("DISP-001", 1209600)` (14 days, max) | Accept, pause for 14 days | ✓ Accept | ✓ Accept |
| `pause_dispute("DISP-001", 1209601)` (14 days + 1 sec) | **Reject** with `DisputePauseDurationExceedsMax` | ✗ Accept (WRONG) | ✓ Reject (CORRECT) |
| `pause_dispute("DISP-001", u64::MAX)` (indefinite) | **Reject** with `DisputePauseDurationExceedsMax` | ✗ Accept (WRONG) | ✓ Reject (CORRECT) |
| Admin compromise → attacker calls `pause_dispute(..., u64::MAX)` | **Blocked** by duration validation | ✗ Escrow frozen indefinitely (WRONG) | ✓ Rejected; governance can respond faster (CORRECT) |

---

## Environment Context

- **Contract version:** Schema version 7+ (dispute pause feature introduced in v7).
- **Location:** `escrow/src/lib.rs`, `pause_dispute()` function (line ~6973).
- **Related files:**
  - `escrow/src/lib.rs` → `is_dispute_paused()` function (checks active pause).
  - `docs/DEPLOYER_SECURITY.md` → operational guidance for dispute pause monitoring.
  - Error definitions in `escrow/src/lib.rs` (enum `EscrowError`).
- **Testing:** New test in `escrow/src/tests/admin.rs` (dispute pause test suite).

---

## Acceptance Criteria

### 1. Code Implementation
- [ ] Add `MAX_DISPUTE_PAUSE_DURATION_SECS = 14 * 24 * 60 * 60` constant.
- [ ] Add `EscrowError::DisputePauseDurationExceedsMax = 170` error code.
- [ ] Add validation check in `pause_dispute()` before timestamp overflow check.
- [ ] No changes to `resume_dispute()`, `is_dispute_paused()`, or `get_dispute_pause()`.

### 2. Testing
- [ ] **Happy path:** `pause_dispute("ticket-1", 3600)` succeeds.
- [ ] **Boundary success:** `pause_dispute("ticket-2", MAX_DISPUTE_PAUSE_DURATION_SECS)` succeeds.
- [ ] **Boundary failure:** `pause_dispute("ticket-3", MAX_DISPUTE_PAUSE_DURATION_SECS + 1)` fails with error 170.
- [ ] **Extreme value rejection:** `pause_dispute("ticket-4", u64::MAX)` fails with error 170.
- [ ] **Negative/zero rejection (existing test):** `pause_dispute("ticket-5", 0)` fails with error 169.
- [ ] **All pause-blocked operations still work after pause expires:** Confirm `fund`, `settle`, `withdraw`, `claim_investor_payout` resume after `get_dispute_pause().expires_at` ledger time passes.
- [ ] **Event emission unchanged:** `DisputePausedEvt` still emitted with correct `paused_at` and `expires_at` times.

### 3. Documentation
- [ ] Update `docs/escrow-error-messages.md` with error code 170 definition and recovery action.
- [ ] Update `docs/DEPLOYER_SECURITY.md` §3.1 to include dispute pause duration monitoring.
- [ ] Add comment in `escrow/src/lib.rs` above `MAX_DISPUTE_PAUSE_DURATION_SECS` explaining the rationale.
- [ ] Update README.md emergency pause section if any.

### 4. CI & Verification
- [ ] `cargo build` succeeds.
- [ ] `cargo clippy -p karis-ky_escrow -- -D warnings` passes with no new warnings.
- [ ] `cargo test` passes (all existing + new tests).
- [ ] Code coverage for new validation path ≥ 95% (via `cargo llvm-cov`).
- [ ] TypeScript SDK (`sdk-ts`) updated to reflect error code 170 (contract spec).

### 5. Schema Version (if applicable)
- [ ] **No schema version bump required** — the new error code is additive; existing escrows can still be paused with any duration value they previously could set (the new max is a forward-looking guard).
- [ ] Confirm `SCHEMA_VERSION` remains unchanged; this is a behavioral enhancement within v7, not a breaking migration.

---

## Security Considerations

### Defense In-Depth Layers

1. **Input validation (this PR):** Enforce `duration_secs <= MAX_DISPUTE_PAUSE_DURATION_SECS` at the contract level.
2. **Operational monitoring:** Alerting on `DisputePausedEvt` with long durations (> 7 days).
3. **Governance multisig:** Admin key is held by M-of-N multisig; single key compromise cannot unilaterally pause indefinitely.
4. **Legal hold orthogonality:** Dispute pause is independent from legal hold; two separate admin controls cannot compound an indefinite freeze.

### Risk Residual After Fix

- **Admin compromise still possible,** but with bounded damage: indefinite pause no longer possible; max pause window is 14 days, forcing governance escalation.
- **Governance slow-roll risk:** If governance cannot respond within 14 days, they can redeploy or migrate state to a new contract (existing procedure).

### Impact on Existing Deployments

- **No breaking change:** Existing pauses with durations ≤ 14 days are unaffected.
- **Validation only affects new `pause_dispute` calls:** Old paused escrows continue to auto-expire at their stored `expires_at` timestamp.
- **Zero-downtime rollout:** Can deploy new contract version without migrating existing instances.

---

## Implementation Notes

### Validation Order

The check must occur **before** `load_escrow_require_admin()` to fail fast on invalid input:

```rust
pub fn pause_dispute(env: Env, ticket_id: String, duration_secs: u64) {
    // Order: 1. ticket_id empty, 2. duration positive, 3. duration max, 4. admin auth
    ensure(&env, !ticket_id.is_empty(), EscrowError::DisputeTicketIdEmpty);
    ensure(&env, duration_secs > 0, EscrowError::DisputePauseDurationNotPositive);
    ensure(&env, duration_secs <= MAX_DISPUTE_PAUSE_DURATION_SECS, EscrowError::DisputePauseDurationExceedsMax);
    
    let escrow = Self::load_escrow_require_admin(&env); // Auth check after input validation
    // ...
}
```

### Error Code Allocation

Error codes are append-only per the project error policy. Code 206 is assigned to `DisputePauseDurationExceedsMax`; code 170 is already assigned to `CloneNotSettled`.

### Constants Placement

Define `MAX_DISPUTE_PAUSE_DURATION_SECS` near other storage/operational constants in `lib.rs`, e.g., alongside `MAX_ATTESTATION_APPEND_ENTRIES = 32` and `MAX_DUST_SWEEP_AMOUNT = 100_000_000`.

---

## Out of Scope

- Making `MAX_DISPUTE_PAUSE_DURATION_SECS` configurable at `init()` time (reserved for future governance upgrade).
- Changing the auto-expiration behavior (still ledger-time-based; no TTL shortening).
- Modifying `resume_dispute()` auth or functionality.
- Legal hold timeout guard (separate issue; would require distinct ADR and design).

---

## References

- **ADR-004:** Legal / compliance hold mechanism → dispute pause as orthogonal feature.
- **DEPLOYER_SECURITY.md:** Post-deployment monitoring section (§3.1).
- **Error code reference:** `docs/escrow-error-messages.md`.
- **Contract spec:** `sdk-ts/spec.json` (must be regenerated after code changes).

---

## Assignee & Timeline

**Assigned to:** [Engineering team]  
**Target completion:** Next sprint (estimated 3–5 days including testing + review)  
**Deployment gate:** All acceptance criteria met + 2 independent code reviews.

