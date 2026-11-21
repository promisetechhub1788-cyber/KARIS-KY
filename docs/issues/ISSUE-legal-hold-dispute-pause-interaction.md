# Issue: Test Legal Hold and Dispute Pause Interaction When Both Are Active

**Type:** Test / security regression
**Status:** Backlog, ready for review and assignment
**Priority:** High
**Severity:** High
**Related:** [Escrow legal hold](../escrow-legal-hold.md), [State machine](../state-machine.md), [Dispute pause design](../../DISPUTE_PAUSE_CHANGES.md), [Escrow error messages](../escrow-error-messages.md)

## Description

Add focused interaction tests for the two independent risk-control overlays:
`DataKey::LegalHold` and the time-bounded dispute pause. The current test suites
exercise each control separately, but the contract must also behave
predictably when an administrator activates both controls on the same escrow.

Both controls are intended to block risk-bearing operations. Activating one
control must not clear, mask, or weaken the other. Clearing the legal hold must
leave an active dispute pause effective, and resuming or allowing a dispute
pause to expire must leave an active legal hold effective.

The test contract should also define deterministic error precedence when both
checks would reject the same call. The current implementation checks legal
hold before dispute pause, so the expected result is the operation-specific
`LegalHoldBlocks*` error while both are active. Any change to that precedence
must be intentional, documented, and applied consistently across entrypoints.

## Current Behavior

Legal hold is a persistent boolean compliance gate controlled by the admin. It
blocks funding, settlement, withdrawal, investor claims, and terminal dust
sweeps. Dispute pause is a separate admin-controlled pause with an expiry
computed from ledger time; it blocks funding, settlement, withdrawal, and
investor claims while active.

The implementation checks the legal-hold guard first and then the dispute-pause
guard in the overlapping risk-bearing entrypoints. Existing tests cover the
controls independently, including pause expiry and manual resume, but there is
no focused matrix proving that simultaneous activation and independent clearing
preserve the intended defense-in-depth behavior.

## Steps to Reproduce

1. Initialize an escrow with a funded state and a participating investor so
   settlement, withdrawal, and claim paths can be exercised.
2. Activate a legal hold with `set_legal_hold(true, reason)`.
3. Activate a dispute pause with `pause_dispute(reason, duration)` while the
   legal hold remains active. Choose a duration long enough to test manual
   clearing and expiry boundaries.
4. Confirm both `get_legal_hold()` and `is_dispute_paused()` report active.
5. Attempt each overlapping operation: `fund` where the escrow is open,
   `settle` while funded, `withdraw` while funded, and
   `claim_investor_payout` after settlement. Use separate fixtures where the
   lifecycle state requires it.
6. Record the typed error, state, balances, claim markers, histories, and
   events after each failed call.
7. Clear the legal hold while the dispute pause is still active. Repeat the
   operations and confirm they remain blocked by the corresponding
   `DisputePausedBlocks*` error.
8. Resume the dispute pause, or advance ledger time to its expiry, while the
   legal hold remains active. Repeat the operations and confirm they remain
   blocked by the corresponding `LegalHoldBlocks*` error.
9. Clear/resume both controls and verify the operation succeeds when all
   unrelated preconditions are satisfied.
10. Repeat the sequence in the opposite order, activating the pause first and
    legal hold second, to prove activation order does not change final state or
    guard behavior.

## Expected Behavior

- Both controls can be active simultaneously and are independently observable.
- Every overlapping risk-bearing operation remains blocked while both controls
  are active.
- While both are active, legal hold takes precedence because it is currently
  evaluated first; each operation returns its specific `LegalHoldBlocks*`
  error and does not fall through to business-state validation.
- Clearing legal hold alone does not unblock an operation if the dispute pause
  is still active; the operation returns its `DisputePausedBlocks*` error.
- Resuming the dispute pause or allowing it to expire alone does not unblock an
  operation if legal hold remains active; the operation returns its
  `LegalHoldBlocks*` error.
- Once both controls are inactive, valid operations proceed normally, subject
  to authorization, lifecycle, maturity, token-balance, and other guards.
- Failed operations are atomic: no escrow status, funding amount, settled
  amount, claim marker, history, token balance, or successful event changes.
- Pause expiry is based on ledger timestamp (`now >= expires_at`), not on an
  explicit resume call, and expiry must not clear the legal hold.
- Clearing or reactivating either control is admin-authorized and preserves the
  state of the other control.

## Actual Behavior

The repository has separate legal-hold and dispute-pause checks and separate
unit tests, but lacks an interaction test demonstrating behavior when both
flags are active. Without this coverage, a future change could reorder guards,
clear the wrong storage key, treat one control as an override, or make behavior
depend on whether legal hold or pause was activated first.

The missing coverage leaves uncertainty around the returned error when both
controls are active and around the intermediate states created by clearing one
control. It also does not currently prove atomicity and event behavior across
those transitions.

## Proposed Solution

1. Add a dedicated interaction test module or a focused section in the
   existing admin/legal-hold tests. Prefer typed `try_*` calls and exact
   `EscrowError` assertions over `should_panic` where both errors are possible.
2. Build a reusable fixture that can create open, funded, and settled escrows,
   then activate both controls and inspect both getters.
3. Test the operation matrix:

   | Operation | Required lifecycle state | Both active | Legal hold cleared | Pause resumed/expired |
   |---|---:|---|---|---|
   | `fund` | open | `LegalHoldBlocksFunding` | `DisputePausedBlocksFunding` | legal hold error |
   | `settle` | funded | `LegalHoldBlocksSettlement` | `DisputePausedBlocksSettlement` | legal hold error |
   | `withdraw` | funded | `LegalHoldBlocksWithdrawal` | `DisputePausedBlocksWithdrawal` | legal hold error |
   | `claim_investor_payout` | settled | `LegalHoldBlocksInvestorClaims` | `DisputePausedBlocksInvestorClaims` | legal hold error |

4. Test both pause release mechanisms: `resume_dispute()` before expiry and
   automatic expiry at exactly `expires_at`. Include one second before expiry
   to prove the pause remains active at the boundary.
5. Assert no state, balance, claim, history, or successful operation event is
   produced after each blocked call.
6. Test idempotent activation and independent toggling: activating an already
   active hold or pause must not alter the other control; clearing one must not
   clear the other.
7. Document guard precedence and the interaction matrix in the state-machine,
   legal-hold, dispute-pause, error, and operator documentation.
8. Run the interaction tests against the supported Rust/Soroban toolchain and
   verify the deployed WASM matches the tested source.

## Environment Context

- **Repository:** `KARIS-KY`
- **Contract:** `escrow` Soroban smart contract
- **Platform:** Stellar Soroban
- **Language/toolchain:** Rust, Cargo, Soroban SDK
- **Controls:** `set_legal_hold`, `clear_legal_hold`, `pause_dispute`,
  `resume_dispute`
- **Read APIs:** `get_legal_hold`, `is_dispute_paused`, `get_dispute_pause`
- **Legal-hold storage:** `DataKey::LegalHold`
- **Dispute-pause storage:** pause state and expiry in instance storage
- **Legal-hold errors:** operation-specific codes 102, 120, 123, 125, and 30
- **Dispute-pause errors:** operation-specific codes 165, 166, 167, and 168
- **Time model:** dispute pause is active while `ledger.timestamp() < expires_at`
- **Authorization:** admin authorization is required to activate or clear either
  control; configured multisig paths must preserve the same semantics
- **Verification:** focused Cargo tests plus full escrow test suite and
  deployed-artifact/hash verification

## Acceptance Criteria

- [ ] A funded/settled fixture can activate legal hold and dispute pause on the
      same escrow and both read APIs report active.
- [ ] `fund`, `settle`, `withdraw`, and `claim_investor_payout` are each tested
      while both controls are active, with exact legal-hold error precedence.
- [ ] Clearing legal hold alone leaves each operation blocked by its dispute
      pause error.
- [ ] Resuming or expiring the dispute pause alone leaves each operation
      blocked by its legal-hold error.
- [ ] Clearing/resuming both controls allows valid operations to succeed.
- [ ] Activation order does not affect control state, error precedence, or
      operation results.
- [ ] Manual pause resume and automatic expiry are both tested, including the
      exact expiry boundary and one second before expiry.
- [ ] Failed operations make no state, token, claim-history, claim-marker, or
      successful-event changes.
- [ ] Legal-hold clear delay rules remain enforced while dispute pause state is
      active, and dispute pause controls cannot clear legal hold state.
- [ ] Idempotent activation and independent toggling are covered for both
      controls.
- [ ] Direct, delegated, and multisig-gated overlapping operations are either
      covered or explicitly documented as out of scope with follow-up issues.
- [ ] State-machine, error, operator, and pause/legal-hold documentation state
      the simultaneous-control behavior and guard precedence.
- [ ] The interaction tests pass against the release artifact and deployed WASM
      verification is recorded before the issue is closed.

## Assignment Notes

Before assignment, confirm whether legal-hold precedence is a stable API
contract or only an implementation detail. Review whether any operational
playbook assumes that clearing one control unfreezes an escrow, and update that
playbook if necessary. Include an explicit decision on whether delegated and
multisig entrypoints must have byte-for-byte equivalent guard ordering or only
equivalent observable outcomes.
