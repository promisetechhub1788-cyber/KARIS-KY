# ADR-004: Legal / Compliance Hold

**Status:** Accepted  
**Date:** 2026-03-28  
**Refs:** `escrow/src/lib.rs` — `propose_legal_hold`, `confirm_legal_hold`, `set_legal_hold`, `clear_legal_hold`, `DataKey::LegalHold`

---

## Context

Regulatory or compliance events may require freezing an escrow mid-lifecycle without destroying state. The mechanism must be governance-controlled and must not be bypassable by any other role.

## Decision

A boolean stored under `DataKey::LegalHold` (defaults to `false` when absent). Escrows initialized through the legacy `init` have no guardian and retain single-step admin activation through `set_legal_hold(true, reason)`. A guardian may be bound at initialization through `init_with_guardian`; in that configuration, activation requires both:

1. `propose_legal_hold(admin)` authorized by the current full admin.
2. `confirm_legal_hold(guardian)` authorized by the configured, admin-distinct guardian before the proposal expires.

Proposals expire after 3,600 seconds of ledger time. Re-proposal by the admin replaces an earlier pending proposal. The immediate activation paths `set_legal_hold(true, ...)` and `set_legal_hold_multisig(..., true, ...)` reject guardian-configured escrows, so they cannot bypass confirmation. Clearance remains admin-only through `set_legal_hold(false, ...)` / `clear_legal_hold()` and retains the configured clear-delay behavior.

When active, the following entrypoints panic immediately:

- `fund` / `fund_with_commitment`
- `settle`
- `withdraw`
- `claim_investor_payout`
- `sweep_terminal_dust`

Read-only entrypoints (`get_escrow`, `get_contribution`, etc.) are never blocked.

There is no hold-duration timelock or automatic expiry — clearing always requires an explicit admin call. The one-hour expiry applies only to pending activation proposals. Production deployments should use a multisig or DAO as `admin` and protect any guardian independently.

## Consequences

- A hold can be applied at any lifecycle stage, including open (blocks new funding) and funded (blocks settlement and claims).
- A guardian is an activation co-approver, not a recovery authority; it cannot clear a hold or recover a lost admin key.
- `LegalHoldChanged` event is emitted on every set/clear so indexers can reconstruct hold history.
- **Terminal state guard (v7+):** As of schema version 7, `set_legal_hold` rejects escrows in terminal states (settled, withdrawn, cancelled, archived; status >= 2). This prevents misleading hold state on already-completed escrows where the hold cannot have operational effect.
- **Guardian XDR change (v8):** `InvoiceEscrow::guardian` changes the stored XDR layout. Existing instances require redeployment; no in-place migration is implemented. Legacy `init` remains ABI-compatible and stores `None`.

## Rejected alternatives

- **Timelock on hold:** adds complexity and a false sense of safety; governance should decide duration.
- **Two-step hold clearance:** rejected; clearance remains recoverable through admin rotation and does not require guardian approval.
- **Ledger-level multisig:** rejected; authorization uses Soroban `require_auth` for the configured guardian rather than imposing Stellar account-level signer policy.
