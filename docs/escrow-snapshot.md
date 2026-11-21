# Funding Close Snapshot

The `FundingCloseSnapshot` is a critical piece of the karis-ky Escrow contract's audit trail. It captures the exact state of the escrow at the moment it transitions from `Open` (0) to `Funded` (1).

## Purpose

This snapshot serves as the **immutable source of truth** for off-chain pro-rata calculations. When an invoice is over-funded (which is allowed by the contract), the full `funded_amount` at the threshold-crossing deposit becomes the denominator for investor share calculations, even when it is greater than `funding_target`.

By capturing this state once and making it immutable, the contract ensures that subsequent actions (like SME withdrawals or settlements) do not shift the relative weight of investor contributions.

## Structure

The snapshot is stored under `DataKey::FundingCloseSnapshot` and contains:

- `total_principal`: The sum of all principal contributed at the moment the funding target was met or exceeded. This equals `InvoiceEscrow.funded_amount` at close and can be greater than `funding_target`.
- `funding_target`: The original target for the invoice.
- `closed_at_ledger_timestamp`: The ledger timestamp when the snapshot was captured.
- `closed_at_ledger_sequence`: The ledger sequence number when the snapshot was captured.

## Dual-Field Record Rationale (Timestamp + Sequence)

Both `closed_at_ledger_timestamp` and `closed_at_ledger_sequence` are stored in the snapshot, even though only `closed_at_ledger_timestamp` is currently used in on-chain maturity comparisons. This redundancy serves several critical purposes:

### 1. **Defensive Consistency Validation**

The contract includes a debug assertion that verifies `closed_at_ledger_sequence == env.ledger().sequence()` at the moment the snapshot is written. This assertion catches scenarios where ledger state might be corrupted or inconsistent, preventing a silent record that mixes time from one ledger with sequence from another.

### 2. **Off-Chain Timeline Reconstruction**

Off-chain tools and indexers may reconstruct the funding timeline by comparing snapshots with the broader ledger history. Having both fields allows them to:
- Cross-check the timestamp against the sequence number for the ledger in which the funding close occurred.
- Detect if ledger time appears to be skewed relative to normal block progression (e.g., a large time jump with only one sequence increment could indicate clock drift or network anomalies).
- Validate that the funding close ledger's time and sequence are internally consistent.

### 3. **Preventing Off-Chain Calculation Errors**

On networks where ledger time is artificially skewed (e.g., due to validator clock misalignment or network partition recovery), using only the timestamp to reconstruct funding intervals could produce incorrect duration calculations. With both fields, off-chain systems can apply additional heuristics:
- Reject funding intervals that appear physically impossible (e.g., a 1-second close interval marked with a 1-hour timestamp delta).
- Flag records for manual review if the sequence progression doesn't match the timestamp progression over a larger time window.
- Use sequence as a fallback ordering mechanism if timestamps are found to be unreliable.

### 4. **Future-Proof Auditing**

If the contract logic ever needs to evolve to use `closed_at_ledger_sequence` for maturity gating or other conditions, the field is already present and populated consistently. This avoids the need for a migration and ensures historical snapshots are complete.

## Lifecycle and Immutability

1. **Before close**: `get_funding_close_snapshot()` returns `None` while the escrow is still open and below target.
2. **Creation**: The snapshot is created during `fund` or `fund_with_commitment` only when `status == 0` and the new `funded_amount >= funding_target`.
3. **Over-funding capture**: If the threshold-crossing deposit overshoots the target, `total_principal` records the full over-funded close amount.
4. **Write-Once**: Once the snapshot is written, the contract's logic prevents it from being updated or overwritten. Later funding attempts are rejected because the escrow is no longer open, and later lifecycle writes do not touch `DataKey::FundingCloseSnapshot`.
5. **Persistence**: The snapshot survives all state transitions, including `settle` and `withdraw`.

## Auditing

Integrators can use the `get_funding_close_snapshot` getter to retrieve this metadata. For historical auditing, the `EscrowFunded` event emitted during the snapshot creation contains the `funded_amount` and `status: 1`, allowing off-chain systems to reconcile the snapshot with the event stream.

The `closed_at_ledger_timestamp` and `closed_at_ledger_sequence` fields are captured from the same ledger as the threshold-crossing funding call. Off-chain indexers should use those fields as the canonical close boundary for pro-rata reporting.

## Security Considerations

1. **Time and Sequence Bounds**: The snapshot captures `env.ledger().timestamp()` and `env.ledger().sequence()`. In Soroban, these are provided by the host environment and are reliable for on-chain time-based logic. Off-chain systems should treat these as the canonical boundaries for the "funded" state transition.
2. **Sequence Consistency Assertion**: At snapshot write time, the contract includes a debug assertion `debug_assert_eq!(closed_at_ledger_sequence, env.ledger().sequence())` to verify that both fields are captured from the same ledger. This prevents silent inconsistencies where the snapshot might record mismatched time and sequence values, which could lead to off-chain calculation errors.
3. **Write-Once Denominator**: `DataKey::FundingCloseSnapshot` is only set if it does not already exist. State transitions such as `settle` and `withdraw` do not recompute the denominator, which prevents later writes from changing investor weights.
4. **State-Machine Misuse**: Funding after close is rejected by the `status == 0` funding guard before contribution or snapshot state can be mutated.
5. **Overflow and Amount Guards**: Funding uses positive amount checks and checked arithmetic before writing `funded_amount` or contribution records.
6. **Token Economics and Assumptions**: As detailed in `escrow/src/external_calls.rs`, this contract strictly assumes standard SEP-41 token mechanics. Malicious, rebasing, or fee-on-transfer (FOT) tokens are **explicitly out of scope** and will trigger safe-failure panics at the balance-check boundaries. This ensures that the `total_principal` captured in the snapshot matches standard token accounting assumptions, preserving the integrity of off-chain payout calculations.
