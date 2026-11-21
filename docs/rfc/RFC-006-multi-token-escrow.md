# RFC-006: Multi-Token Escrow Support

**Status:** Draft  
**Author:** Platform Team  
**Date Proposed:** 2026-07-27  
**Target Release:** TBD  
**Related:** [RFC-001: Multi-Token Support](./RFC-001-multi-token-support.md), Issue #125

---

## Summary

RFC-001 explores multi-token support at a high level, but the current contract
architecture binds exactly one funding token at `init()` and never changes it.
This RFC formally evaluates the engineering path for allowing multiple funding
tokens (e.g., USDC + EURC) within a single escrow or across a sharded escrow
fleet. It documents three design alternatives — the token basket approach, the
shard-per-token approach, and the registry abstraction approach — along with
their tradeoffs and the open questions that must be resolved before a decision
is finalized.

This RFC is a **Draft**. It does not implement multi-token support and does not
finalize a decision.

---

## Motivation

**Problem Statement:**

Each escrow instance is bound to a single funding token at initialization. This
constrains workflows:

1. **Single-currency constraint:** A seller in the EU must accept USDC even if
they prefer EURC.
2. **Conversion friction:** Integrators must manage token swaps outside the
   contract.
3. **Market limitations:** Cannot offer escrows in currencies matching customer
   portfolios.
4. **Operational overhead:** Separate contract deployments per token instead of
   configuration.

**Why now:**

- Stellar ecosystem is expanding (EURC and other stablecoins).
- Partner requests for multi-currency settlement are accumulating.
- Storage schema evolution provides a natural migration point.
- No technical blockers identified at the design level.

**Use Cases:**

1. **International invoicing:** Seller in Mexico receives USDC, settles in USDM.
2. **Regional preference:** European integrators prefer EURC for compliance/tax.
3. **Portfolio matching:** Investor holds EURC; settles in the same asset.
4. **Hedging:** SME denominated in EUR; settles in EURC rather than USDC.

**Success Metric:**

- 3+ distinct funding/settlement tokens active on mainnet within 3 months of
  launch.
- Multi-token settlement > 25% of total escrow volume by the following quarter.

---

## Design Alternatives

### Alternative 1: Token Basket

**Approach:** A single escrow accepts funding in any token from a configured
basket (allowlist). Principal is tracked per token, and settlement pays out
pro-rata across the basket (or in a designated settlement token).

**Pros:**

- Single escrow instance serves all supported tokens.
- No fleet fragmentation; one set of state and events.
- Natural fit for pro-rata yield distribution.

**Cons:**

- Accounting complexity: per-token principal, yield, and payout tracking.
- Cross-token conversion requires an oracle or explicit rate at settlement.
- Larger storage footprint and more validation paths per call.
- Harder to reason about invariants (mixed-token funding target).

### Alternative 2: Per-Token Shards

**Approach:** Deploy one escrow (or shard) per funding token. A coordinating
layer groups shards that share an invoice, and settlement is performed per
shard.

**Pros:**

- Minimal changes to the existing single-token contract.
- Strong isolation: a bug or bad token in one shard cannot affect others.
- Simple accounting — each shard keeps the current invariants.

**Cons:**

- Fleet fragmentation: many deployments to manage and monitor.
- Cross-shard coordination for a single invoice is non-trivial.
- Aggregated reporting and yield accounting span multiple contracts.
- Higher operational and deployment overhead.

### Alternative 3: Registry Abstraction

**Approach:** Introduce a registry contract that maps tokens to metadata and
validates them at runtime. The escrow queries the registry to resolve and
validate funding/settlement tokens instead of hardcoding them.

**Pros:**

- Highly extensible: add tokens without upgrading the escrow.
- Centralized governance over which tokens are supported.
- Reusable across escrow instances and other contracts.

**Cons:**

- Extra host function call on funding/settlement (gas cost).
- Registry becomes a critical dependency and trust assumption.
- Potential race conditions if the registry changes mid-transaction.
- Adds a new contract to the audit surface.

---

## Tradeoffs

| Dimension | Token Basket | Per-Token Shards | Registry Abstraction |
| --- | --- | --- | --- |
| Contract changes | High | Low | Medium |
| Accounting complexity | High | Low | Medium |
| Operational overhead | Low | High | Medium |
| Extensibility | Medium | Low | High |
| Isolation / blast radius | Low | High | Medium |
| New trust assumptions | Oracle (for conversion) | None | Registry contract |
| Gas cost per call | Medium | Low | Medium–High |

---

## Open Questions

1. Should a single escrow support multiple funding tokens, or is a sharded
   fleet sufficient?
2. How is cross-token conversion handled — oracle, explicit rate at settlement,
   or no conversion (same-token only)?
3. What is the migration path from the current single-token schema?
4. How are yield and pro-rata payouts computed across mixed-token principal?
5. Who governs the supported token set, and how are tokens added/removed?
6. What are the failure modes if a token is frozen, depegged, or unresponsive?
7. Does the registry abstraction warrant a separate RFC for its own design?

---

## See Also

- [RFC-001: Multi-Token Support](./RFC-001-multi-token-support.md)
- [RFC Index](./README.md)
