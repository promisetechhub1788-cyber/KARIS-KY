# Issue: `pause_dispute` called twice overwrites expiry without emitting a warning event

**Type:** Bug / behavioral consistency
**Status:** Needs full specification → Ready for assignment
**Priority:** Medium
**Severity:** Low (observability issue, not a correctness or safety regression)
**Related:** [Dispute pause design](../DEPLOYER_SECURITY.md), [State machine](../state-machine.md), [Error messages](../escrow-error-messages.md), [ADR-004 legal hold](../adr/ADR-004-legal-hold.md)

---

## Summary

The `pause_dispute(ticket_id, duration_secs)` entrypoint silently overwrites an active dispute pause when called on an escrow where a pause is already active. The implementation:

1. Does **not** check if a pause is already active
2. **Unconditionally** updates `DataKey::DisputePaused` with the new expiry
3. **Always** emits a `DisputePausedEvt` with `action = 1` (paused)
4. Does **not** emit a warning or advisory event (e.g., "re-paused" or "pause replaced")

This creates an observability gap: an operator or auditor reading the event stream cannot distinguish between an initial pause activation and a subsequent pause reactivation/renewal with a different ticket ID or duration. The old pause state is lost silently.

---

## Description

### Current Behavior

When `pause_dispute` is called:

```rust
// pause_dispute @ escrow/src/lib.rs:7462-7505
let pause_state = DisputePauseState {
    ticket_id: ticket_id.clone(),
    paused_at_ledger_timestamp: now,
    expires_at_ledger_timestamp: expires_at,
};

env.storage()
    .instance()
    .set(&DataKey::DisputePaused, &pause_state);  // ← Unconditional overwrite

DisputePausedEvt {
    name: symbol_short!("disppause"),
    invoice_id: escrow.invoice_id.clone(),
    ticket_id,
    action: 1, // 1 = paused
    paused_at: now,
    expires_at,
}
.publish(&env);
```

**If an active pause exists**, the old `DisputePauseState` is replaced entirely:
- The old `ticket_id` is lost
- The old `paused_at_ledger_timestamp` is lost
- The old `expires_at_ledger_timestamp` is overwritten with a new value
- A single `DisputePausedEvt` with `action = 1` is emitted (same as initial pause)

**Example scenario:**

```
T=1000: admin calls pause_dispute("TICKET-001", 3600)
        → DisputePausedEvt published with action=1, expires_at=4600
        → DataKey::DisputePaused stored: ticket_id="TICKET-001", expires_at=4600

T=2000: admin calls pause_dispute("TICKET-002", 7200)  // Same escrow
        → OLD state (TICKET-001, expires_at=4600) is deleted
        → NEW state (TICKET-002, expires_at=9200) is written
        → DisputePausedEvt published with action=1, expires_at=9200
        → Audit trail shows TWO separate "paused" events, but no indication that the first was overwritten
```

An auditor or operator observing this event stream has no way to know:
- Whether the first pause was intentional (and is now invalidated)
- What the original ticket ID and expiry were
- Whether the second pause was meant to extend, replace, or cancel the first

### Impact on Operators

1. **Audit trail ambiguity:** Compliance and risk teams cannot reconstruct the exact sequence of pause activations and their original durations.
2. **Dispute tracking loss:** If the original `TICKET-001` is referenced in an external dispute system, its pause record is lost and no event documents the replacement.
3. **Debugging difficulty:** When investigating escrow freezes, operators cannot determine whether a pause was overwritten or if the original pause expired naturally.
4. **Operational error recovery:** If an admin accidentally calls `pause_dispute` with the wrong duration, the first call is silently replaced—an operator may not realize the mistake.

---

## Steps to Reproduce

### Step 1: Set up an escrow and activate a dispute pause

```bash
# CLI or SDK call
admin = Address::generate()
sme = Address::generate()
investor = Address::generate()
token = Address::generate()
treasury = Address::generate()

escrow.init(admin, "INV-001", sme, 100_000, 500, 0, token, None, treasury, ...)
escrow.fund(investor, 100_000)  # → status = 1 (funded)

# Pause 1: pause for 1 hour with TICKET-001
escrow.pause_dispute("TICKET-001", 3600)

# Read the pause state
pause_state_1 = escrow.get_dispute_pause()
# → { ticket_id: "TICKET-001", expires_at: T+3600 }

# Capture the event emitted
event_1 = last_event(DisputePausedEvt)
# → { action: 1, ticket_id: "TICKET-001", expires_at: T+3600 }
```

### Step 2: Call `pause_dispute` again with a different ticket and duration

```bash
# Pause 2: pause for 2 hours with TICKET-002 (still same escrow, still paused state active)
escrow.pause_dispute("TICKET-002", 7200)

# Read the pause state again
pause_state_2 = escrow.get_dispute_pause()
# → { ticket_id: "TICKET-002", expires_at: T+7200 }
# ^ NOTE: TICKET-001 is gone, no event logged the replacement

# Capture the event emitted
event_2 = last_event(DisputePausedEvt)
# → { action: 1, ticket_id: "TICKET-002", expires_at: T+7200 }
# ^ Same action=1 as event_1; no distinction visible
```

### Step 3: Verify the old pause state is irretrievable

```bash
# Try to recover the original pause
# → get_dispute_pause() only returns TICKET-002 state
# → Event stream shows two action=1 events
# → No way to determine if TICKET-001 pause was intentional or a mistake
```

---

## Expected vs. Actual Behavior

| Aspect | Expected | Actual |
|--------|----------|--------|
| **Idempotent call (same ticket, same duration)** | No-op or explicit error | Overwrites with identical state; emits event |
| **Reactivation (different ticket/duration)** | Emit warning event or error; require explicit `resume` first | Silently overwrites; emits normal "paused" event |
| **Audit trail** | Clear record of all pause replacements | Gap: old pause state deleted, no "replaced" event |
| **Operator observability** | Distinct event type for re-pause or advisory log | Only action=1 ("paused"); same as initial activation |

---

## Proposed Solution

### Option A: Reject re-pause with explicit error (recommended for operational clarity)

**Behavior:** If a dispute pause is already active, `pause_dispute` returns an error (`EscrowError::DisputePauseAlreadyActive`) instead of silently replacing it.

**Rationale:**
- Forces explicit cleanup: admin must call `resume_dispute()` before reactivating
- Prevents accidental overwrites
- Makes the audit trail clear: each pause activation is intentional
- Aligns with pattern used elsewhere (e.g., `bind_primary_attestation_hash` is single-write)

**Implementation:**
```rust
pub fn pause_dispute(env: Env, ticket_id: String, duration_secs: u64) {
    // ... existing param validation ...
    
    let existing_pause: Option<DisputePauseState> = env
        .storage()
        .instance()
        .get(&DataKey::DisputePaused);
    
    ensure(
        &env,
        existing_pause.is_none(),
        EscrowError::DisputePauseAlreadyActive,  // NEW ERROR
    );
    
    // ... rest of implementation ...
}
```

**New error code:** `EscrowError::DisputePauseAlreadyActive` (append-only, e.g., code 179)

**Test coverage:**
- [ ] `pause_dispute` called twice on active pause returns `DisputePauseAlreadyActive`
- [ ] First pause state is unchanged after failed second call
- [ ] Event emitted only on successful first call, not on rejected second call
- [ ] No state, balance, or timestamp changes occur on rejection

---

### Option B: Emit advisory event on re-pause (less disruptive, retains audit trail)

**Behavior:** If a dispute pause is already active, overwrite it as today, but emit an advisory event (`DisputePauseReplacedEvt`) documenting the old state before the new one.

**Rationale:**
- Backward compatible: existing workflows that call `pause_dispute` multiple times continue to work
- Preserves audit trail: both old and new pause states are logged
- Allows operators to detect unintended overwrites by monitoring for the replacement event

**Implementation:**
```rust
pub fn pause_dispute(env: Env, ticket_id: String, duration_secs: u64) {
    // ... existing param validation ...
    
    let existing_pause: Option<DisputePauseState> = env
        .storage()
        .instance()
        .get(&DataKey::DisputePaused);
    
    // If a pause is already active, emit a replacement advisory before overwriting
    if let Some(old_state) = existing_pause {
        DisputePauseReplacedEvt {
            invoice_id: escrow.invoice_id.clone(),
            old_ticket_id: old_state.ticket_id.clone(),
            old_expires_at: old_state.expires_at_ledger_timestamp,
            new_ticket_id: ticket_id.clone(),
            new_expires_at: // computed below
        }
        .publish(&env);
    }
    
    // ... rest of implementation (overwrite as before) ...
}
```

**New event type:** `DisputePauseReplacedEvt` (advisory)

**Test coverage:**
- [ ] `pause_dispute` called twice emits both `DisputePauseReplacedEvt` (advisory) and `DisputePausedEvt` (activation)
- [ ] Advisory event documents old ticket ID, old expiry, new ticket ID, new expiry
- [ ] Pause state is updated correctly
- [ ] Event ordering: replacement event precedes activation event

---

### Option C: Hybrid—warn on same-ticket idempotent calls, reject cross-ticket overwrites

**Behavior:**
- If called with the **same** ticket ID and duration: emit idempotent advisory, do nothing
- If called with a **different** ticket ID or duration: return error (require explicit resume first)

**Rationale:**
- Allows safe re-assertion of an active pause (same ticket = idempotent)
- Prevents accidental cross-ticket replacements (different ticket = likely mistake)
- Balances backward compatibility with safety

**Implementation:** Conditional check before overwrite

**Test coverage:** Combination of A and B plus cross-ticket scenario

---

## Environment Context

- **Repository:** `KARIS-KY`
- **Contract:** `escrow` (Soroban smart contract)
- **Affected function:** `LiquifactEscrow::pause_dispute(ticket_id, duration_secs)`
- **Storage:** `DataKey::DisputePaused`
- **Related events:** `DisputePausedEvt` (action: 0=resumed, 1=paused)
- **Related functions:** `resume_dispute`, `is_dispute_paused`, `get_dispute_pause`
- **Existing tests:** 
  - `test_dispute_pause_auto_expire_then_settle` (auto-expiry path)
  - `test_dispute_pause_expiry_boundary` (boundary conditions)
  - `test_dispute_pause_blocks_fund_auto_expires` (blocking during pause)
  - `test_dispute_pause_blocks_settle_*` (multiple blocking scenarios)
  - `test_dispute_pause_manual_resume_before_expiry` (manual resume path)
  - **Missing:** `test_pause_dispute_called_twice_*` (re-pause scenarios)
- **Language:** Rust, Soroban SDK (v20+)
- **Error codes:** Append-only enum, new code to be assigned (suggest 179+)

---

## Acceptance Criteria

**Given option A is chosen (error on re-pause):**

- [ ] **Implementation:** `pause_dispute` checks for existing active pause before overwriting
  - Verify logic in `escrow/src/lib.rs:pause_dispute`
  - New error code added to `EscrowError` (append-only, documented in error-messages reference)

- [ ] **Test: Reject re-pause with typed error**
  - Calls `pause_dispute("TICKET-001", 3600)` successfully
  - Calls `pause_dispute("TICKET-002", 7200)` on same escrow with active pause
  - Assertion: second call returns `EscrowError::DisputePauseAlreadyActive`
  - Assertion: first pause state unchanged (still "TICKET-001", expires_at still T+3600)
  - Assertion: exactly one `DisputePausedEvt` emitted (from first call only)
  - Assertion: no state/balance/timestamp changes from second call
  - File: `escrow/src/tests/admin.rs` (new test function)

- [ ] **Test: Resume then re-pause succeeds**
  - Calls `pause_dispute("TICKET-001", 3600)`
  - Calls `resume_dispute()`
  - Calls `pause_dispute("TICKET-002", 3600)` — must succeed (no active pause)
  - Assertion: second `DisputePausedEvt` emitted with new ticket ID
  - File: `escrow/src/tests/admin.rs` (new test function)

- [ ] **Test: No state changes on rejection**
  - Create funded escrow with active pause
  - Attempt re-pause and capture returned error
  - Verify escrow status, funded amount, settlement state unchanged
  - Verify `get_dispute_pause()` returns original pause state
  - Verify no token transfers or balance changes occur
  - File: `escrow/src/tests/admin.rs` (new test function)

- [ ] **Documentation updates**
  - [ ] Docstring in `pause_dispute` explicitly states: "Returns error if pause is already active; call `resume_dispute()` first"
  - [ ] [Error code reference](../escrow-error-messages.md) documents new error code with recovery action
  - [ ] [Operator runbook](../OPERATOR_RUNBOOK.md) includes section: "Re-pausing a dispute" with workaround (resume → re-pause)
  - [ ] [State machine](../state-machine.md) or text description clarifies dispute pause as non-reentrant during active state

- [ ] **Verification: CI pass**
  - `cargo test -p karis-ky_escrow` passes all tests including new ones
  - `cargo clippy -p karis-ky_escrow -- -D warnings` passes
  - `cargo llvm-cov -p karis-ky_escrow --fail-under-lines 95` maintains coverage threshold

- [ ] **Release notes**
  - [ ] Changelog entry: `pause_dispute` now rejects calls when pause already active (prevents silent overwrite)
  - [ ] Migration note: if operators have automated retry logic on `pause_dispute`, they may need to add `resume_dispute()` or handle the new error

---

**Given option B is chosen (advisory event):**

- [ ] **Implementation:** `pause_dispute` emits advisory event before overwrite
  - New event type `DisputePauseReplacedEvt` defined in `escrow/src/lib.rs`
  - Advisory published before the standard `DisputePausedEvt`

- [ ] **Test: Advisory event on re-pause**
  - Calls `pause_dispute("TICKET-001", 3600)` successfully
  - Calls `pause_dispute("TICKET-002", 7200)` on same escrow with active pause
  - Assertion: both calls succeed
  - Assertion: second call emits `DisputePauseReplacedEvt` (advisory) with old/new ticket IDs and expiry values
  - Assertion: second call emits `DisputePausedEvt` (action=1) after the advisory
  - Assertion: pause state correctly reflects new ticket and expiry
  - Assertion: first pause state is lost (no retrieval possible)
  - File: `escrow/src/tests/admin.rs` (new test function)

- [ ] **Test: Event ordering**
  - Verify `DisputePauseReplacedEvt` is published before `DisputePausedEvt`
  - Verify audit tools can correlate the events

- [ ] **Documentation updates**
  - [ ] Docstring clarifies: "If a pause is already active, the new pause replaces it and a `DisputePauseReplacedEvt` advisory is emitted"
  - [ ] [Error code reference](../escrow-error-messages.md) documents advisory event and its interpretation
  - [ ] [Operator runbook](../OPERATOR_RUNBOOK.md) includes section: "Detecting pause overwrites" (watch for `DisputePauseReplacedEvt`)

---

**Common to all options:**

- [ ] **Regression test: backward compatibility**
  - Verify all existing tests still pass (same behavior for new calls on non-paused escrows)
  - Verify settlement, withdrawal, and other workflows unaffected

- [ ] **Integration test: state machine consistency**
  - Pause in `Open` state, `Funded` state, `Settled` state
  - Verify dispute pause works independently of escrow lifecycle state

---

## Assignment Notes

### Decision Point

Before assignment, **the team must choose Option A, B, or C** based on:

1. **Safety vs. compatibility tradeoff:**
   - Option A (error) is safer (prevents silent overwrites) but may break existing automation
   - Option B (advisory) maintains compatibility but leaves audit gap for operators who don't monitor the new event

2. **Operational workflow:**
   - If operators are expected to re-pause as part of normal workflow → Option C (idempotent same-ticket, reject cross-ticket)
   - If re-pause should only happen via explicit `resume` + `pause` sequence → Option A

3. **Release timing:**
   - Option A requires a breaking-change notice (new error)
   - Options B and C are backward compatible

### Implementation Order

1. Choose solution (recommend **Option A** for clarity and safety)
2. Add new error code to `EscrowError` enum (append-only)
3. Implement logic in `pause_dispute`
4. Add test cases (ensure comprehensive coverage of re-pause scenarios)
5. Update docstrings and error-messages reference
6. Update operator runbook and state-machine documentation
7. Run full CI suite and verify release notes are updated

### Related Issues / Follow-ups

- [ ] After implementation, review `resume_dispute` for symmetric clarity (does it emit an advisory if called multiple times? → should not be possible since it removes the pause)
- [ ] Consider adding a health-check entrypoint that validates invariants (pause state consistency, event log sanity)
- [ ] Consider monitoring / alerting strategy for `DisputePauseReplacedEvt` if Option B is chosen

---

## References

- [Dispute pause introduction in DEPLOYER_SECURITY.md](../DEPLOYER_SECURITY.md)
- [Legal hold / dispute pause interaction issue](./ISSUE-legal-hold-dispute-pause-interaction.md)
- [Error messages reference](../escrow-error-messages.md)
- [State machine diagram](../state-machine.md)
- [Existing pause-dispute tests](../../escrow/src/tests/admin.rs) (lines ~1972–2406)
