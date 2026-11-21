# ADR-012: Investor Storage Sharding

**Status:** Accepted
**Date:** 2025-01-01
**Deciders:** Escrow maintainers
**Related:** `docs/arch/sharding-architecture.md`, `ESCROW_SHARDING_SUMMARY.md`

---

## Context

The escrow contract stores per-investor data (contributions, yields, claim flags) in the primary contract's instance storage. Soroban instance storage has a bounded footprint, and the practical investor cardinality limit is roughly ~10k unique investors before the contract approaches or exceeds that limit. Beyond this point, funding operations can fail or become prohibitively expensive, blocking large-scale deployments.

We need a strategy that removes the practical investor limit while keeping the primary contract lightweight and preserving backward compatibility for escrows that do not need sharding.

## Problem

- Per-investor keys in instance storage scale linearly with investor count.
- Instance storage footprint is bounded, imposing a hard practical ceiling on investor cardinality.
- Aggregate state (amount, target, status, maturity, funded total) must remain authoritative and cheap to read.
- Settlement must be able to verify that aggregated per-investor data is consistent with the primary contract's totals.

## Decision

Move per-investor keys to **persistent storage** and distribute them across **shard contracts** (schema v6).

- The primary escrow contract keeps only aggregate state and a shard registry (`shard_id → contract address`) in instance storage.
- Investors are routed deterministically to a shard via `shard_id = hash(investor) % shard_count`.
- Shard contracts are spawned on-demand when the first investor routes to them and store per-investor data in persistent storage.
- During settlement, the primary queries all shards (`get_shard_aggregate_state()`), aggregates totals, and verifies the result equals the primary's `funded_amount` before proceeding.
- Sharding is optional; escrows created without it operate identically to before.

## Alternatives Considered

1. **In-memory map in the primary contract** — Keeps all investor data in a single contract's memory/instance storage. Rejected: does not remove the instance storage ceiling and does not scale beyond the practical investor limit.
2. **External registry contract** — A separate registry contract holds all investor data. Rejected: introduces a single point of contention and a new trust/coordination dependency, and does not distribute storage load across shards.
3. **Hash-based shard contracts (chosen)** — Distributes per-investor data across multiple persistent-storage shards with deterministic routing, keeping the primary contract lightweight.

## Consequences

### Positive

- Unbounded investor cardinality: per-investor data is distributed across shards in persistent storage.
- Deterministic routing: the same investor always maps to the same shard, so no migration is needed on reshard.
- Minimal primary overhead: the primary stores only aggregate state and the shard registry.
- Backward compatible: non-sharded escrows behave exactly as before.

### Negative / Trade-offs

- **TTL implications:** Persistent storage entries have a TTL and must be extended (bumped) to remain live; shard contracts must manage TTL for per-investor keys, unlike instance storage which lives with the contract.
- **Address non-enumerability:** Persistent storage keys are not enumerable, so the set of investors on a shard cannot be iterated directly; aggregation relies on maintained counters/aggregate state rather than key enumeration.
- **Settlement coordination:** Settlement must query every shard and verify the aggregate matches the primary's `funded_amount`, adding coordination cost and a consistency check.
- **Registry growth:** The shard registry grows as shards are spawned and is never deleted.

## References

- `docs/arch/sharding-architecture.md` — detailed sharding module architecture.
- `ESCROW_SHARDING_SUMMARY.md` — sharding design summary.
- `docs/adr/ADR-010-batch_fund-design.md` — batch fund design (distinct decision).
