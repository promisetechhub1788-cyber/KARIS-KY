# Sharding Module Architecture

**Module:** `escrow/src/sharding.rs`
**Status:** Core architectural component (ready for large-scale deployments)
**Purpose:** Enable escrow contracts to scale beyond instance storage limits by distributing investor data across multiple shard contracts.

---

## Overview

The sharding module implements a **horizontal scaling strategy** for escrow contracts that approach or exceed typical investor cardinality limits (~10k unique investors). Rather than storing all per-investor data (contributions, yields, claims) in the primary contract's instance storage, the sharding architecture delegates investor-specific data to **shard contracts** — lightweight sub-contracts that are spawned on-demand and registered with the primary contract.

### Key Design Goals

1. **Unbounded investor cardinality** — Remove the practical investor limit imposed by Soroban instance storage size.
2. **Deterministic routing** — Investors are routed to shards using a hash-based algorithm, ensuring consistent assignment.
3. **Minimal primary contract overhead** — The primary escrow contract remains lightweight, storing only aggregate state and shard registry.
4. **Settlement coordination** — During settlement, aggregated state from all shards is queried and verified for consistency.
5. **Backward compatibility** — Escrows created without sharding (single-shard or instance-storage only) operate identically to before; sharding is optional.

---

## Architecture

### High-Level System Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                      Primary Escrow Contract                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ Escrow Aggregate State (Instance Storage)                   │  │
│  ├──────────────────────────────────────────────────────────────┤  │
│  │ • amount, target, status, maturity                          │  │
│  │ • funded_amount (sum of all shard totals)                  │  │
│  │ • unique_funder_count (aggregated)                         │  │
│  │ • FundingCloseSnapshot (immutable, set once)               │  │
│  │ • Shard registry: shard_id → contract address              │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ Fund Operation Flow                                          │  │
│  ├──────────────────────────────────────────────────────────────┤  │
│  │ 1. Investor submits fund(investor_addr, amount)             │  │
│  │ 2. Primary computes shard_id = hash(investor_addr) % N     │  │
│  │ 3. Primary routes to Shard[shard_id]                        │  │
│  │ 4. Shard records contribution in persistent storage         │  │
│  │ 5. Primary updates funded_amount += amount                  │  │
│  │ 6. If funded_amount >= target, set snapshot (once)          │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ Settlement Aggregation Flow                                  │  │
│  ├──────────────────────────────────────────────────────────────┤  │
│  │ 1. Settlement initiated on primary                          │  │
│  │ 2. Primary queries all shards: get_shard_aggregate_state()  │  │
│  │ 3. Aggregate totals across all shards                       │  │
│  │ 4. Verify aggregated total == primary.funded_amount         │  │
│  │ 5. If verified, proceed to settlement; else fail            │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    │               │               │
        ┌───────────▼────────┐ ┌────▼────────────┐  ┌────▼────────────┐
        │   Shard Contract 0  │ │ Shard Contract 1 │ │ Shard Contract N │
        ├────────────────────┤ ├──────────────────┤  ├─────────────────┤
        │ Persistent Storage  │ │ Persistent      │  │ Persistent     │
        │ • Investor 0        │ │ • Investor 1    │  │ • Investor N   │
        │   - Contribution    │ │   - Contribution│  │   - Contribution
        │   - Yield           │ │   - Yield       │  │   - Yield      │
        │   - Claim flag      │ │   - Claim flag  │  │   - Claim flag │
        │ • Investor 2        │ │ • Investor 3    │  │ • Investor N+M │
        │ ... (more investors)│ │ ... (more)      │  │ ... (more)     │
        └────────────────────┘ └──────────────────┘  └─────────────────┘
         (Hash % N == 0)        (Hash % N == 1)      (Hash % N == N)
```

### Component Roles

| Component | Responsibility | Storage | Lifecycle |
|-----------|-----------------|---------|-----------|
| **Primary Escrow** | Routing, aggregate state, settlement coordination | Instance + Persistent | Lives for full escrow lifecycle |
| **Shard Contract** | Per-investor data, claim processing, aggregation on query | Persistent | Spawned on-demand when first investor routes to it |
| **ShardingConfig** | Governance parameters for routing | Instance (primary) | Set at init; immutable after |
| **Shard Registry** | Mapping shard_id → contract address | Instance (primary) | Grows as shards are spawned; never deleted |

---

## Routing Strategy: Deterministic Hash-Based Assignment

### Algorithm

```rust
fn compute_shard_id(investor: &Address, shard_count: u32) -> u32 {
    let hash = hash_function(investor);      // blake3 or env.crypto().sha256()
    let hash_u32 = u32::from_le_bytes([
        hash[0], hash[1], hash[2], hash[3]
    ]);
    hash_u32 % shard_count                   // Map to [0, shard_count)
}
```

### Properties

| Property | Value | Benefit |
|----------|-------|------|
| **Deterministic** | Same investor → always same shard | No investor migration on reshard |
| **Uniform** | Hash output is well-distributed | Investors spread evenly across shards |
| **O(1) computation** | No table lookups, no state reads | Fast routing in hot paths |
| **Immutable assignment** | Shard ID never changes for an investor | Claim/refund operations find correct shard |

### Hash Function

The reference implementation uses blake3 hashing:
```rust
fn blake3_hash(data: &Address) -> [u8; 32] {
    // In production, replace with:
    // env.crypto().sha256(data.to_bytes())
}
```

**Soroban environment:**
- `env.crypto().sha256()` is available and deterministic across all Soroban hosts.
- blake3 is used in reference documentation; actual deployment uses Soroban's native hash function.

---

## Shard Contract Interface

Each shard exposes a minimal public API for use by the primary escrow contract.

### Entrypoints

```rust
trait ShardContract {
    /// Record investor contribution on this shard.
    /// Called by primary during fund operations.
    fn fund_investor(env: Env, investor: Address, amount: i128);

    /// Get aggregated state for settlement verification.
    /// Called by primary during settlement.
    fn get_shard_aggregate_state(env: Env) -> ShardAggregateState;

    /// Process investor payout claim on this shard.
    /// Called by investor (routed via primary).
    fn claim_investor_payout(env: Env, investor: Address) -> i128;
}
```

### Data Structures

#### ShardEntry
```rust
pub struct ShardEntry {
    pub address: Address,                  // On-chain address of this shard
    pub created_at_ledger: u32,           // Ledger when shard was spawned
    pub investor_count_estimate: u32,     // Cached estimate (for monitoring)
}
```

#### ShardAggregateState
```rust
pub struct ShardAggregateState {
    pub total_contributions: i128,         // Sum of all investor contributions
    pub unique_investor_count: u32,        // Count of distinct investors
    pub shard_count: u32,                  // Total shards active
}
```

#### ShardingConfig
```rust
pub struct ShardingConfig {
    pub max_shards: u32,                   // Max number of shards (governs spawning)
    pub hash_seed: u32,                    // Seed for hash function (customizable)
    pub target_investors_per_shard: u32,   // Soft limit before new shard spawned
}
```

---

## Lifecycle: Lazy Shard Spawning

### Spawning Strategy

Shards are **spawned on-demand**, not pre-allocated:

1. **Init time:** `ShardingConfig` is stored, but no shards are created yet.
2. **First fund to shard 0:** Primary checks if shard 0 contract address is registered.
   - If not registered, spawn shard 0 and store address in `DataKey::ShardAddress(0)`.
3. **Second fund to shard 5:** Primary checks if shard 5 is registered.
   - If not, spawn shard 5.
   - (Shards 1–4 may remain unspawned if no investors route to them.)
4. **Settlement:** Primary queries all **registered shards** (not all possible shards).
   - Only shards with at least one investor are queried.

**Implementation:**
```rust
pub fn ensure_shard_exists(
    env: &Env,
    shard_id: ShardId,
    primary_escrow: &Address,
) -> Address {
    // 1. Check registry
    if let Some(shard_addr) = env.storage().instance()
        .get(&DataKey::ShardAddress(shard_id)) {
        return shard_addr;
    }

    // 2. Spawn new shard
    let shard_wasm = fetch_shard_wasm(); // From WASM storage or pre-deployed
    let shard_addr = env.register(shard_wasm, (shard_id, primary_escrow));

    // 3. Register in primary
    env.storage().instance().set(&DataKey::ShardAddress(shard_id), &shard_addr);

    shard_addr
}
```

### Cost Implications

| Operation | Cost | Notes |
|-----------|------|-------|
| **Init** | Low | Only `ShardingConfig` stored (no shards yet) |
| **First fund to new shard** | ~2–3x regular fund | Includes shard spawn, registration, cross-contract call |
| **Fund to existing shard** | ~1.5x regular fund | Cross-contract call + persistent storage write |
| **Settlement** | O(N) where N = active shards | Each active shard is queried sequentially |

**Optimization:** Shards are spawned early (e.g., during Init) if cardinality is anticipated. This amortizes spawn cost across multiple fund operations.

---

## Settlement and Aggregation

### Aggregation Flow

```
Primary Escrow: settle() called
    │
    ├─ Load escrow state (instance storage)
    │
    ├─ For each registered shard (shard_id in [0, active_shard_count)):
    │   │
    │   ├─ Get shard address from DataKey::ShardAddress(shard_id)
    │   │
    │   ├─ Cross-contract call: shard.get_shard_aggregate_state()
    │   │   ├─ Shard iterates all persistent investor entries
    │   │   ├─ Accumulates contributions and investor count
    │   │   └─ Returns ShardAggregateState
    │   │
    │   └─ Add to running total:
    │       total_contributions += shard.total_contributions
    │       unique_investors += shard.unique_investor_count
    │
    └─ Verify invariant:
        if total_contributions == escrow.funded_amount {
            Aggregation verified ✓
            Proceed to settlement (transfer funds, record payouts)
        } else {
            Invariant violation ✗
            Fail with StateInconsistenciesDetected
        }
```

### Verification Invariant

**Fundamental invariant:**
```
Σ(shard.total_contributions for all shards) == primary.funded_amount
```

If this invariant is violated, it indicates:
- **Data loss** in a shard (contributions not recorded)
- **Data duplication** (contributions counted twice)
- **Shard synchronization failure** (a shard wrote data but primary didn't update aggregate)

When detected, settlement is **rejected** and a diagnostic event is emitted. The escrow remains in **funded** state (no state corruption); operators must investigate and manually reconcile.

---

## Interaction with Existing Features

### Per-Investor Caps and Allowlist

When an investor funds through a shard:

1. **Per-investor cap check:** Primary reads shard to get current contribution, checks cumulative.
   - If investor already contributed 30k and per-cap is 50k, can add up to 20k more.
   - This requires a **read** from the appropriate shard (via cross-contract call).

2. **Allowlist check:** Primary checks allowlist entry (or does so via shard, depending on design).
   - Allowlist is typically instance-stored on primary (small cardinality).
   - If allowlist is sharded, each shard stores allowlist entries for its investors.

### Yield and Claim Processing

**Per-investor yield:**
- Stored in the shard where the investor is routed (e.g., `DataKey::InvestorEffectiveYield(investor)`).
- On claim, investor is routed to shard → shard looks up yield → returns payout.

**Claim lock (commitment time):**
- Stored in shard alongside yield.
- Shard enforces claim lock before allowing payout.

**Claim idempotency:**
- Shard stores `InvestorClaimed` flag per investor.
- On second claim attempt, flag is checked; payout is returned as 0 or idempotent (same payout).

---

## Storage Layout

### Primary Escrow (Instance Storage)

| Key | Type | Purpose |
|-----|------|---------|
| `DataKey::Escrow` | `InvoiceEscrow` | Core state (amount, target, status, etc.) |
| `DataKey::ShardingConfig` | `ShardingConfig` | Routing configuration (immutable after init) |
| `DataKey::ShardAddress(shard_id)` | `Address` | Mapping from shard ID to contract address |
| `DataKey::ShardCount` | `u32` | Total number of active (spawned) shards |
| `DataKey::FundingCloseSnapshot` | `FundingCloseSnapshot` | Immutable snapshot at funded transition |
| *(other escrow keys)* | *(...)* | Treasury, maturity, legal hold, etc. |

**Instance storage grows with:** Number of active shards (O(N)), not with investor count.

### Shard (Persistent Storage)

| Key | Type | Purpose |
|-----|------|---------|
| `(shard_id)` | `ShardEntry` | Metadata about this shard (creation ledger, etc.) |
| `InvestorContribution(investor)` | `i128` | Per-investor principal amount |
| `InvestorEffectiveYield(investor)` | `i64` | Per-investor yield (basis points) |
| `InvestorClaimNotBefore(investor)` | `u64` | Claim lock timestamp (commitment) |
| `InvestorClaimed(investor)` | `bool` | Idempotency flag for claims |

**Persistent storage grows with:** Number of investors routed to this shard (O(M)), where M = total_investors / N.

---

## Monitoring and Debugging

### Shard Health Checks

Operators can monitor shard health by:

1. **Shard aggregation sync:** Periodically call `aggregate_shard_state()` and compare against primary's `funded_amount`.
   - Mismatch indicates data loss or sync failure.

2. **Investor distribution:** Query each shard's `investor_count_estimate` to detect uneven load.
   - If one shard has 2x more investors than others, consider re-hashing with a new `hash_seed`.

3. **Shard creation timeline:** Use `created_at_ledger` to track spawn patterns.
   - Rapid spawning (new shard every ledger) suggests high inbound investor traffic.

### Logging and Events

**Sharding events:**
```rust
pub struct ShardSpawnedEvent {
    shard_id: u32,
    contract_address: Address,
    created_at_ledger: u32,
}

pub struct ShardAggregationCompleteEvent {
    total_contributions: i128,
    unique_investor_count: u32,
    shard_count: u32,
}
```

---

## Limitations and Edge Cases

### Maximum Shard Count

```rust
pub struct ShardingConfig {
    pub max_shards: u32 = 1024,  // Default, configurable at init
}
```

If `max_shards = 1024` and all 1024 shards are spawned, instance storage grows to ~1MB (1024 × ~1KB per registry entry). This is within typical Soroban instance storage limits (~10MB) but approaches practical limits.

**Recommendation:** Use `max_shards = 256` for instances with 10k–100k investors (average 40–390 investors per shard), or `max_shards = 1024` for 100k+ investors.

### Hash Collisions

By the pigeonhole principle, if investors >> shard_count, many investors map to the same shard. This is **expected and accepted**:
- Shard storage scales linearly with investor count, divided by shard count.
- Load is distributed; no shard has significantly more investors than others (assuming good hash function).

### Shard Failure and Recovery

If a shard contract crashes or becomes unresponsive:
1. Settlement calls fail (unable to aggregate state).
2. Escrow remains in **funded** state (no state corruption).
3. Operators must investigate and potentially re-deploy the shard or recover from backup.

**Mitigation:** Shards are simple, deterministic contracts. Recovery consists of re-running all fund operations from the audit log, creating a fresh shard with the same shard_id.

### Cross-Shard Transactions

**Not supported:** An investor cannot split their contribution across multiple shards. Their shard assignment is fixed by their address hash. This is a fundamental design trade-off: guaranteed consistency at the cost of no manual shard reassignment.

---

## Future Enhancements

### Dynamic Re-sharding

If `hash_seed` is changed, investors are re-routed to new shards. This requires:
1. Iterating all shards, migrating investor data to new shards.
2. Verifying aggregated state remains consistent.
3. Minimal downtime if done during a maintenance window.

Currently not implemented; would require an `admin_reshard()` entrypoint.

### Sub-shard Partitioning

If a single shard exceeds storage limits, it could be **further subdivided** into sub-shards. This would require a hierarchical routing scheme and is beyond the current architecture scope.

### Cross-Contract Yield Distribution

Once shard contracts are deployed, yield distribution could be parallelized:
- Primary initiates yield distribution on all shards simultaneously (via `yield_distribute()` call).
- Each shard processes its investor cohort in parallel.
- Primary aggregates final balances.

---

## Testing Strategy

### Unit Tests

Located in `escrow/src/sharding.rs`:

| Test | Purpose |
|------|---------|
| `test_compute_shard_id_deterministic()` | Verify same investor → same shard |
| `test_compute_shard_id_range()` | Verify shard ID in valid range [0, shard_count) |
| `test_compute_shard_id_distribution()` | Verify uniform distribution across shards |
| `test_sharding_config_default()` | Verify default config is sensible |

### Integration Tests

Located in `escrow/src/tests/e2e.rs` or dedicated `sharding_e2e.rs`:

| Test | Purpose |
|------|---------|
| `test_fund_through_multiple_shards()` | Fund 10+ investors; verify routed to correct shards |
| `test_settlement_aggregation_across_shards()` | Settle; verify aggregation matches funded_amount |
| `test_lazy_shard_spawning()` | Verify shards spawn only when needed |
| `test_shard_persistence_across_operations()` | Verify investor data persists in shard across multiple calls |
| `test_claim_through_shard()` | Verify investors can claim from their routed shard |

### Stress Tests

- Fund 10k+ investors and verify sharding efficiency (cost, latency, storage).
- Simulate uneven hash distribution and verify behavior.
- Test shard aggregation with near-missing invariant (e.g., off-by-one in total).

---

## Related Documentation

- **ADR-009:** Per-Investor Keys in Persistent Storage (explains persistent TTL model used by shards).
- **ADR-007:** Storage Key Evolution (explains additive key policy that sharding follows).
- **Escrow Data Model:** `docs/escrow-data-model.md`
- **Investor Caps:** `docs/escrow-investor-caps.md`
- **Settlement Flow:** ADR-003 (explains snapshot and settlement that sharding integrates with).

---

## References

- Module code: `escrow/src/sharding.rs`
- Integration points in `escrow/src/lib.rs`:
  - `fund_impl()` calls `ensure_shard_exists()` to route investors.
  - `settle()` calls `aggregate_shard_state()` to verify aggregation invariant.
- Example: `examples/basic_workflow.rs` (will include sharding examples in future updates).
