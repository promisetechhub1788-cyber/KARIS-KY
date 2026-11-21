# Contract State Export / Import

`export_state` and `import_state` are emergency-use entrypoints that allow
operators to snapshot all enumerable contract state and restore it onto a fresh
contract instance. Their primary use cases are **disaster recovery** and
**network migration** (e.g. testnet → mainnet, or redeploying after
archival/rent expiry on a long-dormant instance).

---

## ⚠️ Danger — read this before calling either entrypoint

These are **not** routine operations. Incorrect use can:

- Reset a funded or settled escrow to an earlier status, locking investor funds.
- Overwrite a live instance with stale state, breaking pro-rata guarantees.
- Silently omit per-investor state if the investor address list is incomplete.

**Only call `import_state` on a freshly deployed, never-initialized contract.**
The entrypoint will reject the call with `ImportAlreadyInitialized` (error 201)
if the target contract already has any `DataKey::Escrow` storage.

---

## What `export_state` captures

`export_state` serializes all **instance-storage** keys into a single
`EscrowSnapshot` struct returned on-chain and emitted in a
`StateExportedEvt` event.

| Captured field | Storage key |
|---|---|
| Core escrow state (`InvoiceEscrow`) | `DataKey::Escrow` |
| Schema version | `DataKey::Version` |
| Funding token address | `DataKey::FundingToken` |
| Treasury address | `DataKey::Treasury` |
| Optional registry hint | `DataKey::RegistryRef` |
| Optional yield tier table | `DataKey::YieldTierTable` |
| Funding-close snapshot | `DataKey::FundingCloseSnapshot` |
| Minimum contribution floor | `DataKey::MinContributionFloor` |
| Max unique investors cap | `DataKey::MaxUniqueInvestorsCap` |
| Per-investor contribution cap | `DataKey::MaxPerInvestorCap` |
| Unique funder count | `DataKey::UniqueFunderCount` |
| Legal hold flag | `DataKey::LegalHold` |
| Legal hold clear delay | `DataKey::LegalHoldClearDelay` |
| Legal hold clearable-at timestamp | `DataKey::LegalHoldClearableAt` |
| Allowlist active flag | `DataKey::AllowlistActive` |
| Primary attestation hash | `DataKey::PrimaryAttestationHash` |
| Attestation append log | `DataKey::AttestationAppendLog` (legacy) or `DataKey::AttestationAppendLogCount` plus `DataKey::AttestationLogEntry(index)` |
| SME collateral commitment | `DataKey::SmeCollateralPledge` |
| Distributed principal | `DataKey::DistributedPrincipal` |
| Funding deadline | `DataKey::FundingDeadline` |
| Pending admin | `DataKey::PendingAdmin` |

### What is NOT captured — per-investor persistent state

The following keys are stored in **persistent storage keyed by investor
`Address`**. Soroban persistent storage is not enumerable, so these keys
**cannot** be included in the export without an explicit investor address list.

| Missing field | Storage key |
|---|---|
| Per-investor contribution | `DataKey::InvestorContribution(Address)` |
| Per-investor effective yield | `DataKey::InvestorEffectiveYield(Address)` |
| Per-investor claim-not-before | `DataKey::InvestorClaimNotBefore(Address)` |
| Per-investor claimed marker | `DataKey::InvestorClaimed(Address)` |
| Per-investor allowlisted flag | `DataKey::InvestorAllowlisted(Address)` |
| Per-investor refunded marker | `DataKey::InvestorRefunded(Address)` |

**You must re-import per-investor state manually.** See the
[Per-investor migration procedure](#per-investor-migration-procedure) section.

---

## Checksum

`export_state` computes a SHA-256 checksum over the following 56-byte
big-endian concatenation of core escrow fields:

```
schema_version(4) | funded_amount(16) | funding_target(16) | yield_bps(8)
  | status(4) | maturity(8)
```

`import_state` recomputes this checksum from the fields in the provided export
and rejects the call with `ImportChecksumMismatch` (error 203) if the values
do not match. This prevents:

- Accidental byte-flip or truncation corruption of the export payload.
- Naive tampering with numeric fields (funded_amount, status, etc.) without
  updating the checksum.

> **Note:** The checksum is an integrity guard, not a signature. A holder of
> the admin key can craft a valid export with arbitrary field values. Protect
> import authority with a multisig admin.

The snapshot includes the optional primary attestation hash and the complete
append-log digest sequence. On import, the digest sequence is restored into the
indexed instance-storage representation used by current contracts, preserving
the log returned by `get_attestation_log`.

---

## Version gate

`import_state` verifies that `export.schema_version == SCHEMA_VERSION` on the
target contract. If the export was produced by a contract running a different
schema version (e.g. v5 export onto a v6 binary), the call fails with
`ImportSchemaMismatch` (error 202).

To migrate across schema versions, upgrade both source and target to the same
WASM before exporting.

---

## Authorization model

| Entrypoint | Who must sign |
|---|---|
| `export_state` | Current `InvoiceEscrow::admin` |
| `import_state` | `export.escrow.admin` (the admin embedded in the export) |

`import_state` authorizes against the admin in the export rather than a stored
admin (since the target has no stored admin yet). This prevents an attacker who
obtains a leaked export from importing it onto a contract they control with a
different admin.

---

## Step-by-step: network migration

### Prerequisites

- Source contract: initialized, all entrypoints have been called as needed.
- Target contract: freshly deployed, `init` has **not** been called.
- Both contracts run the same `SCHEMA_VERSION`.
- You have the admin signing key for the source contract.
- You have a full list of investor addresses that have ever called `fund` or
  `fund_with_commitment` on the source.

### Step 1 — Export instance state

```bash
stellar contract invoke \
  --id <SOURCE_CONTRACT_ID> \
  --network <NETWORK> \
  --source <ADMIN_SECRET_KEY> \
  -- export_state
```

Save the returned `EscrowStateExport` XDR to a file (e.g. `export.xdr`).

### Step 2 — Deploy the target contract

```bash
stellar contract deploy \
  --wasm target/wasm32v1-none/release/karis_ky_escrow.wasm \
  --network <TARGET_NETWORK> \
  --source <DEPLOYER_SECRET_KEY>
```

Note the new contract id.

### Step 3 — Import instance state

```bash
stellar contract invoke \
  --id <TARGET_CONTRACT_ID> \
  --network <TARGET_NETWORK> \
  --source <ADMIN_SECRET_KEY> \
  -- import_state \
  --export <EXPORT_XDR>
```

### Step 4 — Restore per-investor state

For every investor address that ever funded the source contract, read their
state from the source and write it to the target. You need a helper script or
a migration contract; the core reads are:

```bash
# For each investor address $INV:

# Read contribution
stellar contract invoke --id <SOURCE> --network <SRC_NET> \
  -- get_contribution --investor $INV

# Read effective yield
stellar contract invoke --id <SOURCE> --network <SRC_NET> \
  -- get_investor_yield_bps --investor $INV

# Read claim-not-before
stellar contract invoke --id <SOURCE> --network <SRC_NET> \
  -- get_investor_claim_not_before --investor $INV

# Read claimed marker
stellar contract invoke --id <SOURCE> --network <SRC_NET> \
  -- is_investor_claimed --investor $INV
```

Then write these values to the target via your migration helper contract or
a privileged admin-gated restoration entrypoint (not provided in this release;
add one to `migrate` when you implement a concrete migration path).

### Step 5 — Verify

```bash
stellar contract invoke --id <TARGET_CONTRACT_ID> --network <TARGET_NETWORK> \
  -- get_escrow_summary
```

Cross-check `funded_amount`, `status`, `unique_funder_count`, and
`funding_close_snapshot` against the source. Spot-check several investor
contributions.

### Step 6 — Redirect traffic

Update all off-chain systems (indexers, frontend, SDK config) to point to the
new contract id. The old contract can be left in place for read-only historical
queries.

---

## Per-investor migration procedure

Because persistent storage is not enumerable, you must maintain your own
investor address list. Recommended sources for this list:

1. Index all `EscrowFunded` events on the source contract — each event carries
   the investor address.
2. Cross-reference with your off-chain KYC / allowlist records.

For each address:

1. Read `get_contribution`, `get_investor_yield_bps`,
   `get_investor_claim_not_before`, `is_investor_claimed`,
   `is_investor_allowlisted`, `is_investor_refunded` from the source.
2. Write the values to the target via your privileged migration helper.

> Until per-investor state is restored, investors who try to call
> `claim_investor_payout` on the target will get `NoContributionToClaim`
> (error 126) even though the escrow shows status 2 (settled). Restore all
> investor state before announcing the migration to investors.

---

## Error reference

| Code | Constant | Condition |
|---|---|---|
| 200 | `ExportNotInitialized` | `export_state` called before `init` |
| 201 | `ImportAlreadyInitialized` | `import_state` called on an initialized contract |
| 202 | `ImportSchemaMismatch` | `export.schema_version != SCHEMA_VERSION` on target |
| 203 | `ImportChecksumMismatch` | Recomputed checksum differs from `export.checksum` |

---

## Security considerations

| Risk | Mitigation |
|---|---|
| Leaked export used to clone escrow onto attacker-controlled contract | Admin auth required on import; use multisig admin |
| Import resets a live funded escrow | `ImportAlreadyInitialized` guard; import only onto fresh instances |
| Stale per-investor state after migration | Complete Step 4 before directing investors to new contract |
| Replay on wrong network | Verify contract id and network passphrase before signing import |
| Admin key compromise enabling arbitrary import | Use multisig or governance-controlled admin address |

See [`docs/OPERATOR_RUNBOOK.md`](OPERATOR_RUNBOOK.md) for the full redeploy and
upgrade decision tree.
