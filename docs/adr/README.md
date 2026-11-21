# Architecture Decision Records

Key design decisions for the karis-ky escrow contract.

| ADR | Title | Status |
|-----|-------|--------|
| [ADR-001](ADR-001-state-model.md) | Escrow State Model | Accepted |
| [ADR-002](ADR-002-auth-boundaries.md) | Authorization Boundaries | Accepted |
| [ADR-003](ADR-003-settlement-flow.md) | Settlement Flow | Accepted |
| [ADR-004](ADR-004-legal-hold.md) | Legal / Compliance Hold | Accepted |
| [ADR-005](ADR-005-tiered-yield.md) | Optional Tiered Yield and Commitment Locks | Accepted |
| [ADR-006](ADR-006-dust-sweep-and-token-safety.md) | Treasury Dust Sweep and Token Safety | Accepted |
| [ADR-007](ADR-007-storage-key-evolution.md) | Storage Key Evolution and Additive-Key Policy | Accepted |
| [ADR-008](ADR-008-backup-restore-rejection.md) | On-Chain Backup / Restore — Decision and Safe Alternatives | Accepted |
| [ADR-009](ADR-009-per-investor-persistent-storage.md) | Per-Investor Keys in Persistent Storage — TTL, Footprint, and Migration | Accepted |
| [ADR-010](ADR-010-batch_fund-design.md) | Batch Funding Design — Multi-Investor Funding in a Single Call | Accepted |
| [ADR-011](ADR-011-governance-voting.md) | Governance Voting Mechanism | Accepted |

> **Reading order for v5 → v6:** Read [ADR-007](ADR-007-storage-key-evolution.md) (policy, `Rule 5`) first for the high-level additive-key policy; then read [ADR-009](ADR-009-per-investor-persistent-storage.md) for the dedicated rationale, TTL/footprint tradeoffs, and the operator-facing v5 → v6 redeploy plan.

> **Reading order for scaling (10k+ investors):** Read [ADR-010](ADR-010-batch_fund-design.md) for multi-investor funding patterns, then read [ADR-012](ADR-012-investor-storage-sharding.md) for the investor storage sharding decision, and see `docs/arch/sharding-architecture.md` for the full sharding architecture that enables unbounded cardinality.

> **Reading order for governance:** Read [ADR-011](ADR-011-governance-voting.md) for the rationale behind the on-chain voting mechanism, then see [`docs/escrow-governance-voting.md`](../escrow-governance-voting.md) for implementation and operator usage.

Each ADR links directly to the relevant sections of [`escrow/src/lib.rs`](../../escrow/src/lib.rs).
