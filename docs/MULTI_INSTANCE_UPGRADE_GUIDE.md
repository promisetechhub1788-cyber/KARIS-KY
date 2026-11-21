# Multi-Instance Upgrade Guide: Schema Versions v1 → v9

## Overview

This guide documents the upgrade path for KARIS-KY escrow instances as schema versions evolve. Each version transition is either:

- **Additive**: New optional keys are introduced; old instances continue working without redeploy or migration.
- **Breaking**: Existing data structures change; redeploy or migration is required.

**Current Production Version: v9**

---

## Version Transition Matrix

| From | To | Type | Redeploy? | Migrate? | Effort | Notes |
|------|-----|------|-----------|----|--------|-------|
| v1 | v2 | Additive | ❌ No | ❌ No | Low | New investor yield keys; defaults to base yield |
| v2 | v3 | Additive | ❌ No | ❌ No | Low | New snapshot and cap keys; old instances return defaults |
| v3 | v4 | Additive | ❌ No | ❌ No | Low | New attestation keys; old instances have empty log |
| v4 | v5 | Mixed | ⚠️ Review | ❌ No | Medium | Registry ref added; tiered yield available via new entrypoint |
| v5 | v6 | Breaking | ✅ Yes | ❌ No | High | Per-investor keys move from instance → persistent storage |
| v6 | v7 | Additive | ❌ No | ❌ No | Low | Yield claim delegation keys added; no storage location change |
| v7 | v8 | Additive | ❌ No | ❌ No | Low | Additional event types; data structures unchanged |
| v8 | v9 | Additive | ❌ No | ❌ No | Low | New health and yield slippage features; backward compatible |

---

## Detailed Transition Guides

### v1 → v2: Investor Yield Keys (Additive)

**Schema Change**: Adds `InvestorEffectiveYield(Address)` and `InvestorClaimNotBefore(Address)` persistent keys.

**Action**: No action required.

**Testing**:
```rust
// Old v1 instance reads v2 keys as defaults:
assert_eq!(client.get_investor_yield_bps(&investor), 800i64); // base yield
assert_eq!(client.get_investor_claim_not_before(&investor), 0u64); // no lock
```

---

### v2 → v3: Snapshot and Cap Keys (Additive)

**Schema Change**: Adds `FundingCloseSnapshot`, `MinContributionFloor`, `MaxUniqueInvestorsCap`, `UniqueFunderCount`.

**Action**: No action required.

**Testing**:
```rust
// Old v2 instance captures snapshot when funded:
let snapshot = client.get_funding_close_snapshot();
assert!(snapshot.is_some()); // snapshot exists
assert_eq!(snapshot.funded_amount, total_funded);
```

---

### v3 → v4: Attestation Keys (Additive)

**Schema Change**: Adds `PrimaryAttestationHash` and `AttestationAppendLog` instance keys.

**Action**: No action required.

**Testing**:
```rust
// Old v3 instance has empty attestation state:
assert!(client.get_primary_attestation_hash().is_none());
assert_eq!(client.get_attestation_append_log().len(), 0);

// v4 instances can bind attestations:
client.bind_primary_attestation_hash(digest);
```

---

### v4 → v5: Tiered Yield and Registry (Mixed)

**Schema Change**:
- Adds `YieldTierTable(Vec<YieldTier>)` instance key
- Adds `RegistryRef(Address)` instance key
- Adds `fund_with_commitment(investor, amount, lock_secs)` entrypoint

**Action**: Review governance framework for tiered yield rollout; old instances remain unaffected.

**Testing**:
```rust
// v4 instances: no tiers, base yield only
assert_eq!(client.get_investor_yield_bps(&investor), base_yield);

// v5 instances: can use tiered yield
client.fund_with_commitment(&investor, amount, lock_secs);
assert_eq!(client.get_investor_yield_bps(&investor), tier_yield);
```

---

### v5 → v6: Per-Investor Keys to Persistent Storage (Breaking)

**Schema Change**: **Storage location migration** (not XDR shape change).

Per-investor keys are moved from **instance** storage to **persistent** storage:
- `InvestorContribution(Address)` → persistent
- `InvestorEffectiveYield(Address)` → persistent
- `InvestorClaimNotBefore(Address)` → persistent
- `InvestorClaimed(Address)` → persistent
- `InvestorRefunded(Address)` → persistent  
- `InvestorAllowlisted(Address)` → persistent
- `YieldClaimDelegate(Address)` → persistent (v7+)
- `YieldClaimDelegateRevoked(Address)` → persistent (v7+)

**Rationale**: Instance storage is bounded per contract; moving per-investor keys to persistent storage (keyed by address) allows per-address TTL independent of contract instance size.

**Action**: **Redeploy required**.

- Old v5 instances **cannot be migrated** on-chain (no enumeration API to read all instance-stored investor keys).
- Operators must **redeploy fresh instances** at v6.
- Off-chain indexers should migrate investor data to new instances via batch claims.

**Testing**:
```rust
// v6 instances: persistent storage works seamlessly
client.fund(&investor, amount); // writes to persistent
assert_eq!(client.get_contribution(&investor), amount);

// Old v5 instance storage is not readable by v6 code (different location)
// Settlement still works; payout computed from persistent data
client.settle();
let payout = client.compute_investor_payout(&investor);
assert!(payout > 0);
```

**Migration Checklist**:
- [ ] Identify all v5 instances in production
- [ ] Export investor state via `export_escrow_snapshot`
- [ ] Deploy v6 contract on new instance IDs
- [ ] Batch-fund new instances with exported investor state
- [ ] Settle old v5 instances (payouts claimed against old instance)
- [ ] Verify no residual state in old instances
- [ ] Update UI/indexer to point to new v6 instances

---

### v6 → v7: Yield Claim Delegation (Additive)

**Schema Change**: Adds two new persistent storage keys:
- `YieldClaimDelegate(Address)` — per-investor delegate address
- `YieldClaimDelegateRevoked(Address)` — per-investor revocation flag

**XDR Shape**: No changes to existing structs.

**Action**: No action required.

**Compatibility**:
- v6 instances continue working without redeploy.
- Delegation unavailable until investor explicitly sets it.
- Investor yield and claim entrypoints unchanged.

**Testing**:
```rust
// v6 → v7 redeploy (same contract ID, new WASM):
// Old v6 persistent storage keys remain readable
assert_eq!(client.get_contribution(&investor), original_contrib);

// New v7 delegation keys absent by default
assert!(client.get_yield_claim_delegate(&investor).is_none());

// v7 instances can optionally set delegation
client.set_yield_claim_delegate(&investor, &delegate);
assert_eq!(client.get_yield_claim_delegate(&investor), Some(delegate));

// v6 data untouched
assert_eq!(client.get_contribution(&investor), original_contrib);
```

**Features Added**:
- `set_yield_claim_delegate(investor, delegate)` — Investor assigns delegate
- `revoke_yield_claim_delegate(investor)` — Investor revokes delegation
- `claim_investor_payout_as_delegate(investor, delegate)` — Delegate claims on behalf
- `get_yield_claim_delegate(investor)` — Read current delegate
- `is_yield_claim_delegate_revoked(investor)` — Check revocation status

---

### v7 → v8: Event and State Enhancements (Additive)

**Schema Change**: New event types and internal state tracking; no persistent storage changes.

**Action**: No action required.

**Compatibility**: Fully backward compatible; v7 instances unaffected.

---

### v8 → v9: Comprehensive Health and Yield Tracking (Additive)

**Schema Change**: Enhanced health warnings and yield slippage tracking; new entrypoints for state inspection.

**Action**: No action required.

**Compatibility**: Fully backward compatible; v8 instances continue working.

---

## Upgrade Decision Tree

Use this flowchart when deciding how to upgrade an escrow instance:

```
┌─ What version is the instance deployed at?
│
├─ v1–v5
│  ├─ Is it v5?
│  │  └─ YES → See v5→v6 Breaking Migration below
│  └─ NO → Additive-only; can deploy new v9 WASM on same instance ID
│           No migrate() call needed; new features available immediately
│
├─ v6–v8
│  └─ Additive-only chain; deploy new v9 WASM on same instance ID
│     No migrate() call needed
│
└─ v9 (current)
   └─ Already latest; no action needed
```

---

## v5 → v6: Mandatory Redeploy (Detailed Steps)

### Prerequisites
- Export current instance state using `export_escrow_snapshot()`
- Identify all investors and their contributions
- Plan maintenance window (old instance will be settled but not functional for new operations)

### Step 1: Export State
```bash
# Off-chain: Call export_escrow_snapshot on v5 instance
# Captures: escrow metadata, investor contributions, yield tiers, etc.
export_response=$(soroban contract invoke \
  --id <v5_contract_id> \
  -- export_escrow_snapshot)
```

### Step 2: Deploy v6 Instance
```bash
# Deploy new contract (v6 or later WASM)
deploy_response=$(soroban contract deploy \
  --wasm escrow.wasm \
  ...)

new_contract_id=$deploy_response.id
```

### Step 3: Initialize v6 Instance
```bash
# Initialize with same parameters as v5
soroban contract invoke \
  --id $new_contract_id \
  -- init \
  --admin <admin> \
  --invoice_id <invoice_id> \
  --sme <sme> \
  --target <target> \
  --maturity <maturity> \
  --base_yield <base_yield> \
  --funding_deadline <funding_deadline> \
  --token <token> \
  --yield_tiers <yield_tiers_json> \
  --treasury <treasury> \
  --registry_ref <registry_ref>
```

### Step 4: Migrate Investor State (Batch)
```bash
# For each investor in exported state:
soroban contract invoke \
  --id $new_contract_id \
  -- fund \
  --investor <investor_address> \
  --amount <contribution_amount>
```

### Step 5: Settle Old v5 Instance
```bash
# Let investors claim payouts from old instance before settlement
soroban contract invoke \
  --id <v5_contract_id> \
  -- settle
```

### Step 6: Update Routing
- Update off-chain indexer to reference new contract ID
- Update UI endpoints to point to v6 instance
- Publish migration notice to affected users

### Step 7: Verify
```bash
# Confirm v6 instance has all investor state
for investor in $investors; do
  contribution=$(soroban contract invoke \
    --id $new_contract_id \
    -- get_contribution --investor $investor)
  assert_eq $contribution <expected>
done
```

---

## Backward Compatibility Summary

### Always Safe (Additive Upgrades: v1→v4, v6→v9)

- Old instance persists after WASM upgrade to new version
- New optional keys read with `.get(...).unwrap_or(default)`
- Old entrypoints continue to work
- No data migration required
- **Action**: Deploy new WASM; no migrate() call

### Requires Review (Mixed Upgrade: v4→v5)

- New entrypoints available (fund_with_commitment, registry_ref)
- Old entrypoints unchanged
- Old instances unaffected
- **Action**: Review governance framework; gradual rollout if desired

### Requires Redeploy (Breaking Migration: v5→v6)

- Storage location migration (instance → persistent)
- On-chain migration not enumerable
- **Action**: Redeploy fresh instances; migrate investor state off-chain

---

## Testing Upgrade Scenarios

### Test 1: Additive Upgrade (v6 → v7)
```bash
cargo test test_schema_v6_to_v7_additive_delegation_keys
```

### Test 2: v6 Persistent Storage Survives Redeploy
```bash
cargo test test_schema_v6_to_v7_persistent_storage_keys_survive_redeploy
```

### Test 3: Collateral Commitment with Timestamps
```bash
cargo test test_schema_v6_to_v7_collateral_commitment_updates
```

### Test 4: Full Upgrade Matrix (v1 → v9)
```bash
cargo test test_full_version_upgrade_matrix
```

---

## Glossary

- **Additive Upgrade**: New keys/entrypoints added without changing existing XDR shapes; backward compatible.
- **Breaking Migration**: Existing data structures change; redeploy or explicit migration required.
- **Schema Version**: Unique identifier for on-chain data layout; incremented only for breaking changes.
- **Instance Storage**: Soroban contract state keyed by `DataKey::*` (single per-contract entry, bounded footprint).
- **Persistent Storage**: Soroban contract state keyed by `DataKey::*(Address)` (per-address entries, independent TTL).
- **TTL**: Time-to-Live; ledger entries auto-archive if not accessed within TTL window.
- **Redeploy**: Deploy new contract code to a fresh instance ID; old instance remains unchanged.
- **Migrate**: Transform on-chain state from old layout to new layout (used when same instance ID must continue).

---

## Support and Troubleshooting

### Error: `MigrationVersionMismatch`
- Stored version in instance does not match `from_version` parameter to `migrate()`.
- **Action**: Ensure correct version passed; check contract deployment logs.

### Error: `AlreadyCurrentSchemaVersion`
- `from_version >= SCHEMA_VERSION`; instance is already at target version.
- **Action**: No migration needed; can safely upgrade WASM.

### Error: `NoMigrationPath`
- Upgrade path from `from_version` to current `SCHEMA_VERSION` not implemented.
- **Action**: Check ADRs; may require redeploy (e.g., v5→v6).

### Investor Data Missing After Upgrade
- v5→v6 redeploy: Investor state is in old instance, not new instance.
- **Action**: Export state from old v5 instance; batch-fund new v6 instance.

---

## References

- [ADR-007: Storage Key Evolution](../docs/adr/ADR-007-storage-key-evolution.md)
- [OPERATOR_RUNBOOK.md](../docs/OPERATOR_RUNBOOK.md) — Redeploy vs. upgrade decision tree
- [storage-reference.md](../docs/arch/storage-reference.md) — DataKey enum documentation
- [upgrade_compat.rs](../escrow/src/tests/upgrade_compat.rs) — Compatibility tests

