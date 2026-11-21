# BUG-023: fix: get_dispute_pause returns stale data after auto-expiry without resume call

**Issue ID:** BUG-023

**Title:** `fix: get_dispute_pause returns stale data after auto-expiry without resume call`

**Status:** Needs Implementation & Testing

## Description / Summary

After a dispute pause auto-expires based on the `expires_at_ledger_timestamp`, callers invoking `get_dispute_pause()` may receive stale or potentially confusing state data. The expectation is that:
1. Once the pause is logically expired (current ledger timestamp >= expires_at), `get_dispute_pause()` returns `None`
2. The returned state always reflects the current active pause, never a logically-expired but still-stored state
3. Clients and on-chain integrators can safely rely on the return value without manual time-based filtering

## Steps to Reproduce

1. Call `pause_dispute()` with an SME dispute ticket and a specific duration (e.g., 1200 seconds).
2. Record the pause state returned by `get_dispute_pause()` (should be `Some(state)`).
3. Advance the ledger timestamp to exactly the `expires_at_ledger_timestamp` (or beyond).
4. Call `get_dispute_pause()` again at that timestamp or later.
5. **Expected:** `None` is returned.
6. **Actual (potential bug):** A `Some(DisputePauseState)` is returned containing the originally-stored state, even though the pause has logically auto-expired.

## Expected vs. Actual Behavior

### Expected Behavior

- `get_dispute_pause()` performs an expiry check on every invocation, comparing the current ledger timestamp against `expires_at_ledger_timestamp`.
- If `current_timestamp >= expires_at_ledger_timestamp`, the pause is treated as expired and `None` is returned.
- If `current_timestamp < expires_at_ledger_timestamp`, the stored `DisputePauseState` is returned.
- The function **never returns a logically-expired pause state**, even if the storage entry is not explicitly cleaned up.

### Actual Behavior (Potential Issue)

Callers may incorrectly assume that:
1. A previous `get_dispute_pause()` result remains valid across ledger time advances without re-calling the function.
2. The contract automatically clears the storage entry on auto-expiry, leading to confusion if later trying to understand pause history.
3. Calling `resume_dispute()` after auto-expiry should succeed; instead, it may fail with `NoPauseActive` because the storage entry still exists but is logically expired.

## Environment & Context

- **File:** `escrow/src/lib.rs`
- **Functions involved:**
  - `pause_dispute()` — stores `DisputePauseState` with `expires_at_ledger_timestamp`
  - `is_dispute_paused()` — checks if a pause is active (including expiry logic)
  - `get_dispute_pause()` — returns the current pause state if active and not expired
  - `resume_dispute()` — clears the pause (currently does not distinguish expired vs. actively-running pauses)

- **Related structures:**
  - `DisputePauseState` — stores `ticket_id`, `paused_at_ledger_timestamp`, `expires_at_ledger_timestamp`
  - `DataKey::DisputePaused` — persistent storage key for the pause state

## Acceptance Criteria / Definition of Done

### Functional Requirements

1. **Expiry Check Consistency:**
   - `get_dispute_pause()` must check `now < expires_at_ledger_timestamp` on every call.
   - If expired, return `None` regardless of whether the storage entry exists.
   - If active, return `Some(state)`.

2. **No Stale Data Returned:**
   - No internal caching or memoization of pause state across calls.
   - Every invocation of `get_dispute_pause()` must re-read the current ledger timestamp.
   - The returned state, if any, must have `now < expires_at_ledger_timestamp`.

3. **Edge Case: Exactly at Expiry:**
   - When `current_timestamp == expires_at_ledger_timestamp`, the pause is considered expired.
   - `get_dispute_pause()` should return `None` (condition: `now < expires_at` is false).
   - `is_dispute_paused()` should return `false`.

4. **Storage Cleanup (Optional, Documented Behavior):**
   - The storage entry at `DataKey::DisputePaused` is **not automatically cleaned** upon auto-expiry.
   - This is intentional to maintain a historical record and avoid extra storage writes.
   - Clients must rely on the expiry-check logic, not storage presence/absence, to determine pause status.

5. **Resume After Expiry:**
   - If `resume_dispute()` is called after a pause has auto-expired:
     - The storage entry still exists but is logically inactive.
     - `resume_dispute()` should distinguish between "storage entry is None" and "storage entry exists but is expired."
     - **Option A:** Return `NoPauseActive` (current behavior — entry exists, but is expired, so no active pause).
     - **Option B:** Allow cleanup of expired entries and return success (future optimization).

### Testing Requirements

1. **Test: Auto-Expiry Returns None**
   - Pause dispute at time T with duration D.
   - Advance ledger to T+D (exactly at expiry).
   - Call `get_dispute_pause()` — must return `None`.
   - Advance ledger further to T+D+1.
   - Call `get_dispute_pause()` again — must still return `None`.

2. **Test: No Stale Data Across Calls**
   - Call `get_dispute_pause()` at time T (before expiry) — returns `Some(state)`.
   - Advance time to T' > expiry.
   - Call `get_dispute_pause()` again — returns `None`, not stale `Some(state)`.

3. **Test: Resume After Expiry**
   - Pause at T with duration D.
   - Advance to T+D+1 (past expiry).
   - Call `resume_dispute()` — verify behavior (either succeeds with cleanup or fails with `NoPauseActive`).
   - Document the expected outcome and rationale.

4. **Test: Concurrent Expiry Checks**
   - Multiple contract invocations at/around the expiry boundary.
   - Verify `is_dispute_paused()` and `get_dispute_pause()` return consistent results.

5. **Test: Storage Entry Persistence**
   - After auto-expiry, confirm storage entry still exists (`DataKey::DisputePaused` is not removed).
   - Verify this does not leak stale data to callers (functions respect expiry logic, not storage presence).

### Documentation Requirements

1. **Update ADR-004 (or create ADR-008):**
   - Document the auto-expiry and stale-data prevention mechanism.
   - Explain the rationale for not cleaning up storage on auto-expiry.
   - Clarify that expiry is checked on every call, not once at creation.

2. **Code Comments:**
   - Add inline documentation to `get_dispute_pause()` emphasizing the expiry check.
   - Update `resume_dispute()` documentation to clarify behavior with expired pauses.

3. **Operator Runbook:**
   - Add a section on dispute pause lifecycle and auto-expiry.
   - Explain how to interpret empty `get_dispute_pause()` results (could be no pause, or expired pause).

## Implementation Checklist

- [ ] Verify `get_dispute_pause()` performs expiry check on every call (no caching).
- [ ] Verify `is_dispute_paused()` uses consistent logic.
- [ ] Write test for auto-expiry returning `None`.
- [ ] Write test for no stale data across calls.
- [ ] Write test for resume-after-expiry behavior (document expectation).
- [ ] Update ADR documentation.
- [ ] Add code comments.
- [ ] Run full test suite and verify no regressions.

## Potential Root Cause Analysis

The bug report likely stems from:
1. **Client confusion:** Off-chain indexers or applications cache `get_dispute_pause()` results without re-querying after time advancement.
2. **Unclear contract semantics:** The fact that storage entries persist after expiry may lead developers to assume stale data is returned.
3. **Incomplete error handling:** `resume_dispute()` may not clearly distinguish between "no pause ever set" and "pause expired."

## Related Issues & Links

- **ADR-004:** Legal hold and dispute pause mechanism.
- **BUG-012:** (Related auth fix) — `withdraw` SME identity check.
- **Dispute pause feature:** Added in schema version 7.

## Sign-Off

- **Specification Date:** 2026-08-27
- **Status:** Ready for implementation & testing
