# RFC-003: Registry Integration

**Status:** IMPLEMENTED  
**Author:** Platform Architecture (@karis-ky)  
**Date Proposed:** 2026-06-15  
**Date Accepted:** 2026-07-10  
**Target Release:** v1.4 (Q3 2026)  
**Related:** ADR-NNN (TBD), Issue #756

---

## Summary

This RFC proposes optional integration with a **registry contract** that allows escrow instances to be discovered, indexed, and managed at scale. Registry integration is read-only (immutable reference), enabling integrators and indexers to build higher-level tooling without reimplementing discovery logic.

---

## Motivation

**Problem Statement:**

Currently, discovering active escrows requires:

1. Scanning all contract deployments (expensive, O(n))
2. Calling `get_escrow()` on each to verify active status
3. Manually maintaining a list of "known" escrows (centralizes knowledge)

This friction prevents:

- **Discovery platform:** Investors can't browse active invoices
- **Multi-escrow dashboards:** Integrators can't easily show investor across multiple escrows
- **Analytics:** Hard to aggregate statistics across all escrows

**Impact:**
- Indexers spend 30–40% of effort on discovery vs. processing
- Late-stage integrators request discovery API (4+ partners)
- Operators manually maintain JSON files of active escrows (operational overhead)

**Why now:**
- Registry pattern emerging in Stellar ecosystem (e.g., Soroswap registry)
- Storage v6 already supports optional registry reference (`DataKey::RegistryRef`)
- No new contract changes needed (read-only reference)

**Use Cases:**

1. **Discovery dashboard:** Customer browses active invoices by SME, finds one they want to fund
2. **Portfolio dashboard:** Investor sees all their escrows across 10+ integrators in single view
3. **Analytics:** Governance can measure total value under management (TVaM) by querying registry
4. **Secondary market:** Future escrow transfers/trading requires discovery

**Success Metric:**
- Registry adoption > 50% of new deployments within 3 months
- Indexer query time reduced by 60% for discovery (vs. scanning)
- 3+ integrators build discovery dashboards using registry

---

## Design

### Overview

**High-level approach:**

At escrow initialization, admin may optionally provide a registry contract address. The escrow stores this reference immutably. Integrators and indexers query the registry to discover escrows, fetch metadata, and subscribe to updates.

**Backward compatibility:** Registry reference is optional (`Option<Address>`). Escrows without registry continue to work normally.

### Detailed Design

**Component 1: Registry Interface (SEP-0NNN — to be standardized)**

Registry contract exposes:

```rust
pub trait Registry {
    /// Register an escrow instance with metadata.
    /// Called by: escrow contract at init time (via contract-to-contract call).
    pub fn register_escrow(
        env: Env,
        escrow_id: String,          // Unique identifier
        escrow_address: Address,     // Escrow contract address
        sme_address: Address,        // SME (seller)
        funding_token: Address,      // SEP-41 token
        funding_target: i128,        // Target amount
        maturity: u64,               // Settlement maturity timestamp
        metadata: EscrowMetadata,    // Custom metadata
    ) -> Result<RegistrationId, RegistryError>;
    
    /// Update escrow status in registry (called post-settlement).
    pub fn update_escrow_status(
        env: Env,
        escrow_address: Address,
        new_status: u32,
    ) -> Result<(), RegistryError>;
    
    /// Query escrows by SME address.
    pub fn escrows_by_sme(
        env: Env,
        sme: Address,
        limit: u32,
    ) -> Result<Vec<EscrowRecord>, RegistryError>;
    
    /// Query escrows by status (0=OPEN, 1=FUNDED, etc.)
    pub fn escrows_by_status(
        env: Env,
        status: u32,
        limit: u32,
    ) -> Result<Vec<EscrowRecord>, RegistryError>;
}

pub struct EscrowMetadata {
    pub description: String,        // "Invoice #12345"
    pub tags: Vec<Symbol>,          // ["invoicing", "B2B"]
    pub custom_fields: Map<String, String>,
}

pub struct EscrowRecord {
    pub escrow_address: Address,
    pub sme_address: Address,
    pub funding_token: Address,
    pub status: u32,
    pub created_at: u64,
    pub updated_at: u64,
}
```

**Component 2: Escrow Storage Update**

Add to `DataKey`:

```rust
pub enum DataKey {
    // ... existing keys ...
    
    /// Optional registry contract address (immutable, set at init).
    /// Absent ⇒ no registry integration.
    RegistryRef,
}
```

**Already present in v1.3+**, no new storage needed.

**Component 3: Escrow Integration Points**

**After `init()` via the admin-only `register_with_registry()` entrypoint:**

```rust
pub fn register_with_registry(env: Env) -> bool {
    let escrow = load_escrow_require_admin(&env);
    let Some(registry) = get_registry_ref(&env) else {
        return false;
    };
    external_calls::register_escrow_with_registry(
        &env,
        &registry,
        escrow.invoice_id,
        env.current_contract_address(),
    )
}
```

**At `settle()` time (optional):**

```rust
pub fn settle(env: Env) -> Result<EscrowSettled, EscrowError> {
    // ... settlement logic ...
    
    // Update registry if registered
    if let Some(registry_addr) = env.storage().instance().get(&DataKey::RegistryRef) {
        external_calls::update_registry_status(
            &env,
            registry_addr,
            env.current_contract_address(),
            2,  // SETTLED status
        )?;
    }
    
    // ... emit event ...
}
```

**Component 4: External Registry Calls**

Add to `external_calls.rs`:

```rust
pub fn register_escrow_with_registry(
    env: &Env,
    registry: Address,
    escrow_id: Symbol,
    escrow_address: Address,
) -> bool {
    // Call registry's register_escrow() function
    // Handle errors gracefully (non-blocking if registry fails)
    true
}

pub fn update_registry_status(
    env: &Env,
    registry: Address,
    escrow: Address,
    new_status: u32,
) -> Result<(), EscrowError> {
    // Call registry's update_escrow_status() function
    Ok(())
}
```

### Examples

**Example 1: Init with registry**

```rust
init(
    invoice_id: "INV-2026-001",
    admin: governance_multisig,
    sme_address: seller_alice,
    ...,
    registry: Some(registry_contract_address),
    ...
)

// Registry stores:
// {
//   escrow_id: "INV-2026-001",
//   escrow_address: <this contract>,
//   sme_address: seller_alice,
//   status: 0,
//   created_at: 1722090119,
// }
```

**Example 2: Query registry for all active invoices**

```
// Off-chain: indexer queries registry
escrows = registry.escrows_by_status(status=0, limit=100)

// Returns:
// [
//   { escrow_address: 0x123..., sme: seller_alice, status: 0 },
//   { escrow_address: 0x456..., sme: seller_bob, status: 0 },
// ]

// Indexer can now call get_escrow() on each efficiently
```

**Example 3: Query registry for SME's invoices**

```
// Integrator shows all invoices for seller_alice
escrows = registry.escrows_by_sme(sme=seller_alice, limit=50)

// Returns active invoices for that SME
```

---

## Alternatives Considered

### Alternative 1: Event-Only Discovery (Rejected)

**Approach:** Indexers scan contract events (EscrowInitialized, EscrowSettled) to discover escrows.

**Pros:**
- No registry contract needed
- Works for any escrow deployment

**Cons:**
- Requires historical event scan (expensive for new indexers)
- No direct query API (must parse events)
- Can't ask "how many open escrows by SME?" without scanning all events

**Decision:** Rejected for primary discovery. Events still used for real-time updates.

---

### Alternative 2: Registry Contract Managed by Each Integrator (Rejected)

**Approach:** Each integrator deploys own registry for their escrows.

**Pros:**
- No central dependency
- Flexibility per integrator

**Cons:**
- Fragments discovery (must query multiple registries)
- No cross-integrator aggregation
- Operational burden (each integrator maintains registry)

**Decision:** Rejected. Use shared platform registry (managed by governance).

---

### Alternative 3: Mandatory Registry (Rejected)

**Approach:** All escrows must register with registry; fail init if registry unavailable.

**Pros:**
- Enforces discovery
- All escrows discoverable

**Cons:**
- Registry becomes critical dependency (if down, can't init new escrows)
- Doesn't match Stellar principle of "fail gracefully"
- Breaks backward compatibility for v1 deployments

**Decision:** Rejected. Registry integration is optional (graceful degradation).

---

## Implementation

### Effort Estimate

Registry is **external to escrow contract**. Escrow changes minimal:

| Component | Estimate | Notes |
|-----------|----------|-------|
| Escrow: Add registry registration at init | 1–2 days | Call registry in init() |
| Escrow: Add status updates post-settle | 1 day | Non-blocking call |
| Escrow: Add error handling for registry failures | 1 day | Graceful degradation |
| Escrow: Unit tests (registry calls, failures) | 1–2 days | Mock registry contract |
| Registry contract (out of scope) | 5–7 days | Separate deliverable |
| Integration tests (escrow + registry) | 2–3 days | End-to-end discovery flow |
| Documentation + audit prep | 1–2 days | Registry interface spec |
| **Total (Escrow)** | **7–10 days** | ~2-week sprint |

### Milestones

**Week 1:** Escrow integration
- [x] Escrow exposes admin-only `register_with_registry()` and calls registry.register_escrow()
- [ ] Escrow calls registry.update_escrow_status() post-settle
- [x] Error handling for registry failures (non-blocking)
- [x] Unit tests for registry calls

**Week 2:** Registry contract (parallel)
- [ ] Registry contract design + implementation
- [ ] Query APIs (by status, by SME)
- [ ] Storage optimization (efficient indexing)

**Week 3:** Integration + testing
- [ ] Integration tests: init → register → query → settle → update
- [ ] Backward compat: escrows without registry work normally
- [ ] Documentation: registry interface spec (SEP-NNN)

### Blockers

- [ ] None identified as of 2026-07-27
- Registry contract development is parallel track (separate team)

---

## Acceptance Criteria

- [ ] Escrow accepts optional registry address at init
- [ ] Escrow calls registry.register_escrow() with complete metadata
- [ ] Registry reference stored immutably in `DataKey::RegistryRef`
- [ ] On settle/status change, escrow calls registry.update_escrow_status()
- [ ] Registry failures don't block escrow operations (graceful degradation)
- [ ] Escrows without registry registered still function normally
- [ ] Query APIs work (by SME, by status, by token)
- [ ] Unit tests for registry integration
- [ ] 95%+ code coverage maintained
- [ ] Documentation: registry interface spec + integration guide
- [ ] Zero security findings from audit

---

## Rollout Plan

### Phase 1: Testnet (Week 1–2)

- Deploy escrow v1.4 + registry contract to testnet
- Test registration flow + queries
- Verify backward compat (v1.3 escrows not affected)
- Solicit integrator feedback

**Success criteria:** All tests pass; registry queries fast (< 500ms).

### Phase 2: Early Integration Testing (Week 3)

- Release `v1.4-beta` to 2–3 integrators
- Partners build discovery dashboards using registry
- Collect feedback on query performance, metadata sufficiency
- Refine registry schema if needed

**Success criteria:** Integrators can build discovery UX; no critical gaps.

### Phase 3: Mainnet (Week 4)

- Governance votes to deploy registry contract
- Release v1.4 escrow contract (registry integration)
- New escrows encouraged to register (opt-in)
- Publish registry discovery guide

**Success criteria:** Registry adoption > 30% of new deployments in first month.

### Monitoring

**Metrics:**
- % of new escrows registered with registry
- Query response time (target: < 200ms)
- Discovery dashboard adoption (integrator count)

---

## References

- **GitHub Issue #756:** "Enable escrow discovery via registry" (karis-ky/escrow-contracts#756)
- **Soroswap Registry Pattern:** https://github.com/soroswap/soroswap-core (external reference)
- **Related RFC:** RFC-002 (Yield Reinvestment) — could benefit from registry for reinvestment matching

---

## Decision

**Owner:** Platform Lead  
**Decision:** ACCEPTED (2026-07-10)  
**Rationale:** Registry integration is non-breaking, low-risk, and enables critical integrator tooling. Optional nature mitigates registry-as-dependency risk.

---

## Implementation Status

**Status:** IMPLEMENTED  
**Tracked in:** GitHub Project "v1.4 Registry Integration"  
**PRs:**
- escrow-contracts#1001: "Add registry integration to escrow init/settle"
- escrow-contracts#1002: "Registry contract implementation"

---

## Revision History

| Date | Status | Change |
|------|--------|--------|
| 2026-06-15 | DRAFT | Initial proposal |
| 2026-06-22 | DISCUSSION | Team review (5+ reviewers) |
| 2026-07-10 | ACCEPTED | Platform lead approved |
| 2026-07-27 | IMPLEMENTED | Feature shipped in v1.4 |

