# ADR-009: Delta-Encoded State Snapshots

**Status:** Accepted  
**Date:** 2026-07-28  
**Issue:** #217  
**Refs:** `escrow/src/lib.rs` — `SnapshotDelta`, `DataKey::FullSnapshot`, `DataKey::SnapshotDeltaChain`, `reconstruct_snapshot_from_deltas`

---

## Context

Soroban contract storage is a scarce resource. The escrow contract re-writes the full `DataKey::Escrow` state on every transition (fund, settle, withdraw, beneficiary rotation, etc.). For long-lived escrows with many state changes, this results in:

- **Repeated full snapshots**: each field is written even if unchanged.
- **Storage bloat**: multiple copies of identical data across ledger entries.
- **Auditing overhead**: off-chain systems must deduplicate snapshots to reconstruct history.

Delta-encoded snapshots allow the contract to store only **incremental changes**, reducing storage while maintaining an immutable audit trail.

---

## Decision

### 1. New Type: `SnapshotDelta`

Define an incremental state change record:

```rust
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotDelta {
    pub delta_id: u32,                      // Unique, monotonically increasing
    pub recorded_at: u64,                   // Ledger timestamp
    pub based_on_delta_id: u32,             // Previous delta ID (0 = baseline)
    pub funded_amount_delta: i128,          // Signed change
    pub maturity: u64,                      // New value (0 if unchanged)
    pub status: u8,                         // New value (255 if unchanged)
    pub admin: Option<Address>,             // New value (None if unchanged)
    pub sme_address: Option<Address>,       // New value (None if unchanged)
}
```

### 2. New Storage Keys (Additive, per ADR-007)

Three new `DataKey` variants:

```rust
pub enum DataKey {
    // ... existing keys ...

    /// Baseline full snapshot for delta chain reconstruction.
    /// Immutable after set. Absent ⇒ no delta encoding in use.
    FullSnapshot,

    /// Head of the delta chain: delta_id of the latest applied delta.
    /// Absent ⇒ no deltas (only full snapshot).
    SnapshotDeltaChain,

    /// Per-delta storage: SnapshotDelta(delta_id) → SnapshotDelta struct.
    SnapshotDelta(u32),
}
```

### 3. Reconstruction Algorithm

**Load and apply:**

1. Load baseline `FullSnapshot` (or fall back to `Escrow` if deltas not in use).
2. Load `SnapshotDeltaChain` head ID.
3. Walk the delta chain backwards via `based_on_delta_id` until reaching 0 (baseline).
4. Collect all deltas; reverse to chronological order.
5. Apply each delta, mutating the baseline escrow:
   - `funded_amount += delta.funded_amount_delta` (checked arithmetic).
   - If `delta.maturity != 0`: `escrow.maturity = delta.maturity`.
   - If `delta.status != 255`: `escrow.status = delta.status`.
   - If `delta.admin.is_some()`: `escrow.admin = delta.admin`.
   - If `delta.sme_address.is_some()`: `escrow.sme_address = delta.sme_address`.
6. Return reconstructed escrow.

**Immutability guarantee:** Once written under `DataKey::SnapshotDelta(id)`, a delta is never updated or deleted. The chain is append-only.

### 4. Delta Recording

After each state mutation that changes `InvoiceEscrow`:

1. Load previous escrow state.
2. Compute delta by comparing all fields.
3. Assign `delta_id = SnapshotDeltaChain + 1`.
4. Store delta under `DataKey::SnapshotDelta(delta_id)`.
5. Update `SnapshotDeltaChain = delta_id`.

**Optional:** Store full snapshot under `FullSnapshot` on first fund (status 0 → 1) for reconstruction baseline.

### 5. Backward Compatibility

**No schema version bump:** `SCHEMA_VERSION` remains unchanged.

**Existing instances:** continue storing full escrow under `DataKey::Escrow`; no migration required. New instances can opt-in by setting `FullSnapshot`.

**Graceful fallback:** `reconstruct_snapshot_from_deltas()` returns `Escrow` if deltas are not in use.

**Coexistence:** during transition period, both `Escrow` and delta keys may exist; reconciliation tests ensure consistency.

---

## Rationale

### Why deltas?

- **Storage efficiency**: a typical delta occupies ~200–400 bytes; a full escrow snapshot is ~500 bytes. A 5-event chain saves ~700 bytes vs. five full snapshots.
- **Audit trail**: delta chain forms an immutable ledger of changes; off-chain systems can reconstruct history without guessing.
- **No redeploy required**: additive keys (per ADR-007) mean existing instances upgrade in-place.

### Why immutability?

- Prevents tampering: once a delta is recorded, it cannot be edited to hide a prior state.
- Simplifies reconstruction: no need to check for "deleted" or "superseded" deltas.
- Audit safety: external systems can trust the chain is stable.

### Why optional?

- Gradual rollout: new instances adopt deltas; old instances continue unchanged.
- Risk mitigation: if delta reconstruction has a bug, old instances are unaffected.
- Flexibility: future versions may use a different encoding strategy; opt-in allows safe experimentation.

### Why those fields?

The delta tracks the **mutable fields** of `InvoiceEscrow`:
- `funded_amount`: changes on every fund call.
- `status`: changes on status transitions (fund, settle, withdraw, etc.).
- `maturity`, `admin`, `sme_address`: may change via admin/SME calls.

Immutable fields (`invoice_id`, `amount`, `funding_target`, `yield_bps`) are not stored in deltas.

---

## Consequences

### Immediate

- Storage savings for high-activity escrows (20–30% reduction in typical scenarios).
- Immutable audit trail of state changes.
- No forced migration; backward compatible.

### Future Enhancements

- **Automatic compaction**: after N deltas, collapse chain into new `FullSnapshot` + reset chain.
- **Partial deltas**: encode unchanged fields as absent (space optimization).
- **Snapshot queries**: off-chain indexers reconstruct state at any delta ID for historical analysis.
- **Time-travel auditing**: reconstruct escrow state as of block N without re-reading all N deltas.

---

## Compatibility

### Existing Instances

- Upgrade without redeploy: new keys are absent; contract continues using `Escrow` as before.
- Indexers ignore delta keys; snapshots are read from `Escrow` as usual.

### New Instances

- Opt into delta encoding at `init` time (future enhancement: configuration flag).
- Indexers must support delta reconstruction to query historical state.

### Migration

- Non-blocking: no `migrate` call required.
- Operators may manually set `FullSnapshot` and begin delta recording for an old instance if desired.

---

## Testing

### Unit Tests

- `test_delta_chain_basic_creation`: create and verify delta chain after fund.
- `test_delta_reconstruction_after_settle`: reconstruct state post-settlement.
- `test_multiple_deltas_state_transitions`: verify delta chain grows correctly over multiple ops.
- `test_delta_on_beneficiary_rotation`: delta captures beneficiary change.
- `test_delta_storage_concept`: verify deltas are created and tracked.
- `test_backward_compat_no_deltas_required`: old instances work without deltas.
- `test_delta_immutability`: once written, delta cannot be modified.
- `test_escrow_consistency_multiple_ops`: state consistency over many operations.

### Integration Tests

- **Reconciliation**: after every state transition, verify `reconstruct_snapshot_from_deltas()` == `get_escrow()`.
- **Overflow safety**: extreme funded amounts do not cause panic in delta reconstruction.
- **Broken chain detection**: if a delta is missing, reconstruction fails gracefully.

### Fuzz Tests

- Random state transitions; verify delta chain is always consistent.
- Reconstruct at every delta ID; verify monotonic progression.
- Compare reconstructed state vs. full state across 100+ random escrow lifecycles.

---

## Error Handling

New error codes (optional, for future use):

```rust
SnapshotDeltaChainCorrupted = 165,
SnapshotDeltaNotFound = 166,
```

---

## References

- [ADR-007: Storage Key Evolution](ADR-007-storage-key-evolution.md)
- [Escrow Data Model](../escrow-data-model.md)
- [Escrow Pro-Rata Math](../escrow-pro-rata.md)
- [Issue #217](https://github.com/karis-ky/escrow-contracts/issues/217)
