# Schema Version Interoperability Matrix

This matrix documents which schema versions are compatible with each other and defines the upgrade paths for the karis-ky escrow contract.

## Quick Reference

| Schema Version | Status | Key Features | Upgrade Path | Breaking Changes |
|---|---|---|---|---|
| 1 | Deprecated | Initial schema (fund/settle/claim) | N/A | N/A |
| 2 | Deprecated | Per-investor yields, claim locks | Additive | None |
| 3 | Deprecated | Funding snapshot, caps | Additive | None |
| 4 | Deprecated | Attestations | Additive | None |
| 5 | Deprecated | Tiered yields, registry ref | Additive* | Possible struct layout change |
| 6 | Deprecated | Persistent storage for investors | **Redeploy** | Yes — storage location changed |
| 7 | Deprecated | Dispute pause state | Additive | None |
| 8 | **Current** | Guardian for legal-hold two-step confirmation | — | — |

*v5: Redeploy may be required if `InvoiceEscrow` struct layout changed

## Detailed Version Compatibility

### Schema v1 → v2
- **Type:** Additive upgrade
- **Changes:** `InvestorEffectiveYield`, `InvestorClaimNotBefore` keys added
- **Compatibility:** ✓ v2 can read v1 escrows
- **Migration:** No `migrate()` call required

### Schema v2 → v3
- **Type:** Additive upgrade
- **Changes:** `FundingCloseSnapshot`, `MinContributionFloor`, `MaxUniqueInvestorsCap`, `UniqueFunderCount` added
- **Compatibility:** ✓ v3 can read v2 escrows
- **Migration:** No `migrate()` call required

### Schema v3 → v4
- **Type:** Additive upgrade
- **Changes:** `PrimaryAttestationHash`, `AttestationAppendLog` added
- **Compatibility:** ✓ v4 can read v3 escrows
- **Migration:** No `migrate()` call required

### Schema v4 → v5
- **Type:** Conditional upgrade
- **Changes:** `YieldTierTable`, `RegistryRef`, `Treasury` added; `fund_with_commitment` added
- **Compatibility:** ⚠ Depends on `InvoiceEscrow` struct layout changes
- **Migration:** Check struct layout; redeploy if XDR layout changed

### Schema v5 → v6
- **Type:** **Breaking change (Redeploy Required)**
- **Changes:** Per-investor keys moved to persistent storage
- **Compatibility:** ✗ No in-place upgrade path
- **Reason:** Different storage location; addresses not enumerable
- **Migration:** Redeploy to new instance, restore investor data

### Schema v6 → v7
- **Type:** Additive upgrade
- **Changes:** `DisputePaused` state for temporary dispute freezes
- **Compatibility:** ✓ v7 can read v6 escrows
- **Migration:** No `migrate()` call required

### Schema v7 → v8
- **Type:** Additive upgrade
- **Changes:** Optional `InvoiceEscrow::guardian` field for two-step legal-hold activation
- **Compatibility:** ✓ v8 can read v7 escrows
- **Migration:** No `migrate()` call required

## Cross-Version Interaction

### Settlement across versions
- ✓ Latest version can settle escrows created by any earlier compatible version
- ✓ Core settlement logic remains unchanged across versions
- ⚠ Per-version constraints apply (e.g., v1 has no investor caps)

### Investor claims across versions
- ✓ Compatible within same storage model (v1-v5 → v1-v5)
- ✗ Requires data restoration when crossing storage boundaries (v5 → v6)

## Redeploy vs. Upgrade Decision Tree

```
IS struct layout changing? (check git diff on InvoiceEscrow)
├─ YES → REDEPLOY REQUIRED
│   ├─ Deploy new instance
│   ├─ Call init with same parameters
│   ├─ Restore investor data (if applicable)
│   └─ Archive old instance (legal hold)
│
└─ NO → ADDITIVE UPGRADE (safe to use same instance)
    └─ Update WASM, no data migration needed
```

## Operator Checklist

- [ ] Verify current schema version matches `SCHEMA_VERSION` constant in `escrow/src/lib.rs`
- [ ] Consult this matrix before planning an upgrade
- [ ] For breaking changes (v6+), coordinate investor notifications
- [ ] For additive upgrades, no special coordination required
- [ ] Archive old instances with legal hold for compliance

## References

- Source of truth: `SCHEMA_VERSION` constant in `escrow/src/lib.rs`
- Detailed ADRs in `docs/adr/`
- Operator runbook: `docs/OPERATOR_RUNBOOK.md`
