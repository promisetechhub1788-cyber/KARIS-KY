# ADR-010: Batch Funding Design — Multi-Investor Funding in a Single Call

**Status:** Accepted
**Date:** 2026-08-27
**Refs:** `escrow/src/lib.rs` — `LiquifactEscrow::fund_batch`, `MAX_FUND_BATCH`, `DataKey::FundingCloseSnapshot`; `escrow/src/tests/funding.rs` — batch funding tests; Issue #340 (DOCS-009)

---

## Context

The escrow contract allows investors to fund an invoice in phases via the [`fund`](../escrow-sim-stellar-cli.md) entrypoint. However, large-scale escrow instances with many simultaneous investors (e.g., invoice auctions or syndication pools) would require the caller to invoke `fund` separately for each investor, creating multiple round-trips and unnecessary on-chain transaction overhead.

The `fund_batch` entrypoint (introduced in schema version 7+) addresses this by allowing a **single contract call to record multiple investor principals** in atomic fashion, while preserving all per-investor invariants and maintaining precise settlement snapshot semantics.

---

## Decision

### 1. Batch Funding Entrypoint

A new public entrypoint `fund_batch(entries: Vec<(Address, i128)>)` is added alongside the existing [`fund`](../escrow-sim-stellar-cli.md) entrypoint.

```rust
pub fn fund_batch(env: Env, entries: Vec<(Address, i128)>) -> InvoiceEscrow
```

**Parameters:**
- `entries`: A vector of `(Address, i128)` tuples, where each tuple is an investor address and their funding amount.

**Returns:**
- The updated `InvoiceEscrow` state after all entries have been processed.

**Semantics:**
- Each entry is processed sequentially in order.
- Each entry undergoes **full per-investor validation** (auth, caps, allowlist, legal hold, maturity, etc.), as if it had been submitted via individual `fund` calls.
- If any entry fails validation, the entire batch call fails **without corrupting prior entries**; the contract state remains consistent up to the last successful entry before the error.
- **Exactly one** `EscrowFunded` event is emitted per entry (identical to single-fund semantics).
- If the escrow transitions from **open** (status 0) to **funded** (status 1) **within the batch**, the transition occurs at the entry that crosses the funding target, and [`DataKey::FundingCloseSnapshot`](../escrow-snapshot.md) is recorded **exactly once** for that transition. Remaining entries in the batch are processed even after the transition.

### 2. Bounded Batch Size

To prevent unbounded iteration and storage cost, the batch size is capped at:

```rust
const MAX_FUND_BATCH: u32 = 50;
```

**Rationale:**
- Soroban host functions charge per-operation costs. A batch of 50 entries involves ~50 contribution writes plus 50 event publishes, which fits within typical transaction budgets while remaining practical for large-scale funding coordination.
- Larger batches (e.g., 1000+) would risk hitting Soroban's per-transaction limits and block legitimate use cases.
- Callers needing to fund > 50 investors can invoke `fund_batch` multiple times.

### 2a. Single-Read / Single-Write Escrow Pattern

`fund_batch` (and its `batch_fund` alias) does **not** read and write the escrow struct once per entry. Instead, the escrow is loaded **once** before the loop, threaded through the per-entry funding logic as an in-memory value, and persisted **once** after the loop completes.

**Implementation shape:**

```rust
// Load once.
let mut escrow = Self::get_escrow(&env);

for (investor, amount) in entries {
    // fund_impl takes the in-memory escrow and returns the updated
    // in-memory escrow; it performs no escrow storage read or write.
    escrow = Self::fund_impl(&env, escrow, investor, amount)?;
}

// Persist once.
Self::set_escrow(&env, &escrow);
```

**Consequences:**
- The escrow instance key is read exactly once and written exactly once per batch call, regardless of `entries.len()`.
- Per-entry validation (auth, caps, allowlist, legal hold, maturity, KYC, sanctions) is unchanged; only the storage access pattern differs.
- Per-entry contribution records and `EscrowFunded` events are still written/published per entry, so event semantics and contribution accounting are identical to the previous implementation.
- The funded transition and `FundingCloseSnapshot` write still occur at the entry that crosses the target, using the in-memory escrow state.

**Measured storage-op reduction (batch of 50 entries):**

| Storage operation | Before | After |
|-------------------|--------|-------|
| Escrow instance reads | 50 | 1 |
| Escrow instance writes | 50 | 1 |
| Contribution record writes | 50 | 50 |
| Event publishes | 50 | 50 |

This removes 49 redundant escrow reads and 49 redundant escrow writes per full-size batch, with no semantic change to funding behavior.

**Errors:**
- [`EscrowError::FundingBatchEmpty`] (code 73) if `entries.len() == 0`
- [`EscrowError::FundingBatchTooLarge`] (code 74) if `entries.len() > MAX_FUND_BATCH`

### 3. Per-Entry Authorization

Each entry in the batch requires the investor (`Address` field) to authorize the operation via `require_auth()`.

```rust
for (investor, amount) in entries {
    investor.require_auth();
    // ... process funding ...
}
```

**Implication:**
- Even though the batch is submitted in a single call, **each investor must independently sign / approve the transaction** (or the caller must hold a delegation).
- This preserves the per-investor consent model: an investor cannot be blindly funded by a batch submitter without their explicit authorization.

### 4. Snapshot Semantics and Funded Transition

If the escrow is in **open** (status 0) state before the batch:

- **Before first batch entry:** `FundingCloseSnapshot` does not exist (or is `None`).
- **During batch processing:** If entry `i` causes `funded_amount >= funding_target`, the escrow transitions to **funded** (status 1), and `FundingCloseSnapshot` is written with the state at that moment.
- **After transition:** Remaining entries in the batch (entries `i+1`, `i+2`, ...) are **processed normally**, even though the escrow is now **funded**. However, the [`DataKey::FundingCloseSnapshot`] is **immutable** and is **not overwritten** by subsequent entries.

**Example:**
```
Initial state: status=0 (open), funded_amount=0, funding_target=100k

Batch: [(inv1, 40k), (inv2, 55k), (inv3, 10k)]

Processing:
  Entry 0: inv1 funds 40k → funded_amount = 40k (status still 0)
  Entry 1: inv2 funds 55k → funded_amount = 95k (status still 0)
  Entry 2: inv3 funds 10k → funded_amount = 105k (status → 1, snapshot created)
              funded_amount in snapshot = 105k
              
Final state: status=1, funded_amount=105k, snapshot exists and captures final totals
```

This design ensures that **the funding snapshot is recorded exactly at the moment funding is complete**, not before, not after. Settlement can then rely on this snapshot for pro-rata calculations.

### 5. Per-Investor Invariants Preserved

All per-investor checks from the standard [`fund`](../escrow-sim-stellar-cli.md) entrypoint are applied **per entry**:

| Check | Per-Entry? | Rationale |
|-------|-----------|-----------|
| **Investor authorization** (`require_auth()`) | Yes | Each investor must authorize their own entry |
| **Amount > 0** | Yes | Zero-amount entries are rejected |
| **Legal hold active** | Yes | Entire batch fails if legal hold is active |
| **Escrow status == 0 (open)** | Yes | Funding is blocked if escrow is already funded/settled/withdrawn |
| **Min contribution floor** | Yes | Each entry checked against `MinContributionFloor` |
| **Max per-investor cap** | Yes | Each entry checked against `MaxPerInvestorCap`, **cumulative with prior contributions from same investor** |
| **Unique investor cap** | Yes | Each new investor checked against `MaxUniqueInvestorsCap`; duplicates in batch count as one unique investor for cap purposes |
| **Allowlist (if active)** | Yes | Each investor checked if allowlist is enabled |
| **Sanctions screening** | Yes | Each investor screened against configured sanctions provider |
| **KYC verification** | Yes | Each investor verified if KYC registry is configured |

**Important:** The **per-investor cap is cumulative within the batch**. If investor A appears twice in a batch and the per-investor cap is 50k:
- First entry: A funds 30k → cumulative = 30k ✓
- Second entry: A funds 25k → cumulative = 55k ✗ (exceeds cap)

The second entry fails with [`EscrowError::InvestorContributionExceedsCap`], and the batch terminates. Prior entries (including A's first entry) remain recorded.

### 6. Snapshot Immutability

Once [`DataKey::FundingCloseSnapshot`] is written during a batch (or any funding operation), subsequent entries in the same batch do not overwrite it.

**Implementation detail:**
```rust
if escrow.status == 0 && new_funded_amount >= escrow.funding_target {
    // Escrow transitioning to funded
    if !snapshot_already_exists {
        write_funding_close_snapshot(/* ... */);
    }
    escrow.status = 1;
}
```

This ensures that even if multiple entries in the batch cross the target (e.g., a near-funded escrow receiving a batch that continues past 100%), the snapshot captures the state at the **first transition**, not the last.

---

## Rationale

### Why batch funding?

**Efficiency for large-scale coordination:**
- A marketplace or syndication platform coordinating 20+ investors would invoke `fund` 20+ times. Batch funding reduces this to a single call.
- Reduces gas costs: fewer contract invocations, fewer event subscriptions needed on indexers.
- Simplifies off-chain orchestration: a single atomic batch commit instead of sequential fund operations.

### Why strict per-entry validation?

**Safety and auditability:**
- Each entry undergoes the same checks as if it were a standalone `fund` call. There are no "relaxed" rules for batch entries.
- This prevents a malicious or buggy batch submitter from bypassing caps or allowlists with a carefully crafted batch.
- Audit trail remains clear: each investor's consent and each entry's state change are independently verifiable.

### Why limit batch size to 50?

**Bounded cost and fairness:**
- Soroban transactions have fixed gas budgets. A batch of 1000+ entries could exhaust the budget and waste user fees.
- A smaller cap (50) encourages batches to be split across multiple calls, spreading compute cost across multiple ledger entries and preventing any single call from dominating a ledger.
- 50 is large enough for practical use cases (e.g., a VC fund closing a 30-investor syndicate in one call) but small enough to fit in standard Soroban budgets.
- Because the escrow is read once and written once per call (see §2a), the per-entry marginal cost is dominated by the contribution record write and the event publish, so the cap is no longer driven by repeated escrow instance access.

### Why thread the escrow in memory instead of re-reading per entry?

**Cost without semantic change:**
- Re-reading and re-writing the same escrow instance key per entry is pure overhead: the value is already in memory and no other entry in the batch can observe an intermediate persisted state.
- Passing the escrow into `fund_impl` and returning the updated value keeps the per-entry validation and accounting logic in one place while removing the redundant host storage calls.
- The single persist at the end of the loop preserves the existing partial-failure semantics: on error, the batch aborts and the escrow is not written with partially applied in-memory state.

### Why is the snapshot immutable within the batch?

**Deterministic settlement calculations:**
- If the snapshot changed mid-batch, subsequent settlements could calculate pro-rata shares incorrectly.
- By locking the snapshot at the first funded transition, all future entries (in the same batch and beyond) use the same denominator for pro-rata calculations.
- This matches the behavior of standard `fund` calls: the snapshot is set once per escrow, not per call.

### Why per-entry authorization?

**Investor consent and regulatory clarity:**
- A batch that required only one authorization (e.g., from a platform admin) would violate the **per-investor consent principle** encoded in ADR-002 (Authorization Boundaries).
- By requiring each investor to `require_auth()`, we ensure that even in a batch scenario, each investor has signed off on their specific entry.
- This is especially important in regulated environments (KYC, sanctions compliance) where investor-specific consent must be explicit.

---

## Errors and Recovery

### Batch Errors

| Error | Code | Scenario | Recovery |
|-------|------|----------|----------|
| `FundingBatchEmpty` | 73 | `entries.len() == 0` | Caller must provide at least one entry |
| `FundingBatchTooLarge` | 74 | `entries.len() > MAX_FUND_BATCH` | Split batch into multiple calls, each ≤ 50 entries |
| `FundingAmountNotPositive` | 26 | Entry has `amount <= 0` | Caller must ensure all amounts > 0 |
| `InvestorContributionExceedsCap` | 61 | Investor cumulative would exceed per-investor cap | Reduce entry amount or submit in separate batch |
| `MaxUniqueInvestorsCap` | 73 | New investor would exceed unique investor cap | Wait or create new escrow with higher cap |
| `LegalHoldBlocksFunding` | 44 | Escrow is under legal hold | Admin must clear legal hold before batch can proceed |
| `FundingIsPaused` | 78 | Funding velocity auto-pause or manual pause is active | Admin must resume funding |
| *(all other per-fund checks)* | *(see docs/escrow-error-messages.md)* | Allowlist, sanctions, KYC, maturity, etc. | See per-check error documentation |

### Partial Batch Failure

If entry `i` fails validation:
1. Entries `0` through `i-1` are **already recorded** on-chain.
2. Entry `i` is **rejected** and the batch call returns an error.
3. Entries `i+1` through `n-1` are **not processed**.
4. The escrow state reflects the contributions from entries `0` through `i-1` (and any snapshot written during those entries).

**Caller responsibility:** If a batch partially fails, the caller should:
- Inspect the returned error to identify why entry `i` failed.
- Modify entry `i` (reduce amount, add investor to allowlist, etc.) or skip it.
- Resubmit the remaining entries in a new batch.

---

## Testing Strategy

All batch funding tests are located in [`escrow/src/tests/funding.rs`](../../escrow/src/tests/funding.rs) under the comment section `// Tests for fund_batch entrypoint (Issue #311)`.

### Test Coverage

| Test | Purpose | Key Assertions |
|------|---------|-----------------|
| `test_fund_batch_rejects_empty` | Empty batch is rejected | Panics with `FundingBatchEmpty` |
| `test_fund_batch_rejects_oversized` | Oversized batch is rejected | Panics with `FundingBatchTooLarge` |
| `test_fund_batch_equals_n_single_funds` | Batch funding is equivalent to N individual `fund` calls | Final state, contributions, and events are identical |
| `test_fund_batch_per_investor_cap_rejection` | Per-investor cap is enforced per entry | Entry exceeding cap is rejected; prior entries remain |
| `test_fund_batch_mid_batch_funded_transition` | Snapshot is recorded exactly at funded transition | Snapshot exists after batch; reflects correct total |
| `test_fund_batch_duplicate_addresses` | Duplicate investor in batch accumulates contributions | Second entry of same investor is cumulative |
| `test_fund_batch_per_investor_auth` | Each investor must authorize their entry | (Requires custom auth mock; current test uses `mock_all_auths()`) |
| `test_fund_batch_single_entry` | Single-entry batch behaves like `fund` | Contribution and state match expected values |
| `test_fund_batch_max_batch_size` | Batch at exactly `MAX_FUND_BATCH` is accepted | 50 entries succeed; 51st entry in separate batch |
| `test_fund_batch_preserves_event_semantics` | One event per entry | Event count = entry count |

### Invariants Verified

**State invariants after batch:**
- `funded_amount == sum of all contributions recorded in batch`
- `status == 0` (open) if `funded_amount < funding_target`
- `status == 1` (funded) if `funded_amount >= funding_target` (and snapshot exists)
- `FundingCloseSnapshot` present iff status transitioned to 1
- `UniqueFunderCount` incremented exactly once per distinct new investor
- The escrow instance is read once and written once per batch call, independent of batch size

**Storage-op invariants:**
- Escrow instance read count for a batch call is exactly 1
- Escrow instance write count for a batch call is exactly 1
- Contribution record writes and event publishes remain exactly 1 per entry

**Event invariants:**
- One `EscrowFunded` event per entry
- `EscrowFunded` emits the correct investor address and amount for each entry
- If status transitions, one `FundingCloseSnapshot` event is also emitted

---

## Integration Examples

### Marketplace Funding a 5-Investor Syndicate

```rust
// Off-chain: coordination layer has 5 investors and their amounts
let entries = vec![
    (investor_alice, 20_000),
    (investor_bob, 15_000),
    (investor_charlie, 25_000),
    (investor_diana, 22_000),
    (investor_eve, 18_000),
];

// Single on-chain call
let result = escrow_client.fund_batch(&entries);

// Result: escrow is now funded (if total 100k >= target)
assert_eq!(result.status, 1); // Funded
```

### Handling Partial Batch Failure

```rust
let mut entries = vec![/* 50 carefully selected entries */];

match escrow_client.fund_batch(&entries) {
    Ok(result) => {
        println!("All entries processed. Escrow status: {}", result.status);
    }
    Err(EscrowError::InvestorContributionExceedsCap) => {
        // Entry 15 exceeded a per-investor cap
        // Remove that entry and resubmit
        entries.remove(15);
        escrow_client.fund_batch(&entries).ok();
    }
    Err(other) => eprintln!("Batch failed: {:?}", other),
}
```

### Multiple Batches Over Time

```rust
// Day 1: Fund phase 1 (30 investors)
escrow_client.fund_batch(&phase_1_entries); // 30 entries

// Day 2: Fund phase 2 (20 more investors)
escrow_client.fund_batch(&phase_2_entries); // 20 entries

// Result: funded_amount is sum of all 50 investors
// Settlement uses the snapshot from the first batch that crossed the target
```

---

## Compatibility

### Schema Versioning

Batch funding was introduced at **schema version 7**. Earlier schemas do not have the `fund_batch` entrypoint.

**Upgrade path for v6 → v7:**
- Existing v6 instances can continue using `fund` calls.
- New v6 instances cannot call `fund_batch` (entrypoint does not exist).
- Deployment of v7 instances enables `fund_batch` alongside `fund`.

### Backward Compatibility

The `fund_batch` entrypoint does **not** affect existing escrows or existing funding patterns:
- All escrows created before v7 can still use the `fund` entrypoint.
- New escrows can optionally use `fund_batch` for efficiency.
- Single-investor scenarios have no reason to use `fund_batch` (one `fund` call is simpler and equally efficient).

---

## Future Considerations

### Potential Enhancements

1. **Asynchronous batch processing:** If future Soroban updates allow unbounded iteration, we could relax the `MAX_FUND_BATCH` limit or allow "streaming" fund operations.

2. **Batch refunds:** A corresponding `refund_batch` could allow cancelling multiple investor positions in one call (currently requires N `refund` calls).

3. **Batch claims:** Similarly, a `claim_investor_payout_batch` for settlements could reduce indexer overhead when many investors claim simultaneously.

4. **Sharding integration:** Once shard contracts are fully deployed (see ADR-011), `fund_batch` could intelligently route entries to appropriate shards based on investor hash, further optimizing storage and routing costs.

---

## Related ADRs

- **ADR-002:** Authorization Boundaries — explains the per-investor consent model that batch funding preserves.
- **ADR-003:** Settlement Flow — explains the funding-close snapshot that batch funding writes.
- **ADR-007:** Storage Key Evolution and Additive-Key Policy — explains how new entrypoints maintain schema compatibility.

---

## References

- Entrypoint: `LiquifactEscrow::fund_batch` / `batch_fund` in `escrow/src/lib.rs`
- Per-entry funding logic: `LiquifactEscrow::fund_impl` in `escrow/src/lib.rs` (takes and returns an in-memory `InvoiceEscrow`)
- Tests: `escrow/src/tests/funding.rs` (search for `// Tests for fund_batch`)
- Error codes: `docs/escrow-error-messages.md` (codes 73–74)
- Snapshot design: `docs/escrow-snapshot.md`
- CLI usage: `docs/escrow-sim-stellar-cli.md` (fund_batch section)
- Issue tracking: GitHub Issue #311 (batch funding implementation), #340 (DOCS-009, this ADR), #239 (single-read/single-write escrow refactor)
