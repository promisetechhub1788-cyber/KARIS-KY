# Storage Reference

Detailed catalog of `DataKey` variants and contract types.

## Storage Layout Reference

| Key | Type | Storage | Indexed By | Description |
|-----|------|---------|------------|-------------|
| `Escrow` | bool/option | Instance | — | Full escrow snapshot ([`InvoiceEscrow`]); rewritten atomical... |
| `Version` | bool/option | Instance | — | Read with [`LiquifactEscrow::get_version`]. Never delete or ... |
| `InvestorContribution` | Address | Persistent | Address | **Persistent** storage. Absent ⇒ `0`. One entry per investor... |
| `LegalHold` | bool/option | Instance | — | Absent ⇒ `false` (no hold). Toggled by admin via [`Liquifact... |
| `LegalHoldClearableAt` | bool/option | Instance | — | Absent ⇒ no clear request is pending.... |
| `LegalHoldClearDelay` | bool/option | Instance | — | [`LiquifactEscrow::set_legal_hold(env, false)`]. Absent ⇒ `0... |
| `SmeCollateralPledge` | bool/option | Instance | — | Absent when no commitment has been recorded. Replaceable by ... |
| `InvestorClaimed` | Address | Persistent | Address | **Persistent** storage. Absent ⇒ `false`. Written once; a se... |
| `FundingToken` | bool/option | Instance | — | Immutable after init.... |
| `Treasury` | bool/option | Instance | — | Immutable after init.... |
| `RegistryRef` | bool/option | Instance | — | Omitted from storage when unset at init. Absent ⇒ `None`.... |
| `YieldTierTable` | bool/option | Instance | — | **Trust:** values are protocol-supplied at deploy; the contr... |
| `FundingCloseSnapshot` | bool/option | Instance | — | Absent until the escrow reaches `status == 1`. See [`Funding... |
| `InvestorEffectiveYield` | Address | Persistent | Address | **Persistent** storage. Absent ⇒ falls back to [`InvoiceEscr... |
| `InvestorClaimNotBefore` | Address | Persistent | Address | **Persistent** storage. Absent ⇒ `0`. One entry per investor... |
| `MinContributionFloor` | bool/option | Instance | — | Written as `0` even when unconfigured so reads always succee... |
| `MaxUniqueInvestorsCap` | bool/option | Instance | — | Absent ⇒ unlimited. Checked against [`DataKey::UniqueFunderC... |
| `MaxPerInvestorCap` | bool/option | Instance | — | Absent ⇒ unlimited. Checked against [`DataKey::InvestorContr... |
| `PendingAdmin` | bool/option | Instance | — | Absent ⇒ no pending handover. Cleared after successful accep... |
| `UniqueFunderCount` | bool/option | Instance | — | Written as `0` at init; incremented once per new investor in... |
| `PrimaryAttestationHash` | bool/option | Instance | — | Absent until [`LiquifactEscrow::bind_primary_attestation_has... |
| `AttestationAppendLog` | bool/option | Instance | — | Absent ⇒ empty log. See [`LiquifactEscrow::append_attestatio... |
| `AttestationRevoked` | u32 | Instance | u32 | Preserves the original digest for auditability while signall... |
| `AllowlistActive` | bool/option | Instance | — | When true, only allowlisted addresses may call [`LiquifactEs... |
| `InvestorAllowlisted` | Address | Instance | Address | Whether a specific address is permitted to fund when [`DataK... |
| `InvestorRefunded` | Address | Instance | Address | Absent ⇒ `false`. Written once; prevents double-refund.... |
| `DistributedPrincipal` | bool/option | Instance | — | `outstanding = funded_amount - distributed_principal`.... |
| `FundingDeadline` | bool/option | Instance | — | Optional funding deadline (ledger timestamp); after it passe... |
| `YieldSlippageThreshold` | bool/option | Instance | — | Absent ⇒ `0` (no slippage check).... |


## DataKey Storage-Placement Audit

Audit of every `DataKey` variant defined in `escrow/src/lib.rs`, classifying its
storage tier and estimating its approximate serialized size. Sizes are rough
upper bounds for the Soroban XDR encoding of the value (excluding the key
itself); `Address` values are ~40–56 bytes, `i128`/`u64` are 16/8 bytes, and
`bool`/`u32` are 1/4 bytes. This audit is informational only — **no storage
migration is performed in this issue**.

| Key | Storage | Approx. size | Read frequency | Recommendation |
|-----|---------|--------------|----------------|----------------|
| `Escrow` | Instance | ~200–300 B | Every call | Keep in instance; single hot snapshot. |
| `Version` | Instance | ~4 B | Rare | Keep in instance; tiny. |
| `InvestorContribution` | Persistent | ~56 B | Per investor | Already persistent. |
| `LegalHold` | Instance | ~1 B | Frequent | Keep in instance. |
| `LegalHoldClearableAt` | Instance | ~8 B | Rare | Keep in instance; tiny. |
| `LegalHoldClearDelay` | Instance | ~8 B | Rare | Keep in instance; tiny. |
| `SmeCollateralPledge` | Instance | ~80 B | Rare | Keep in instance; small. |
| `InvestorClaimed` | Persistent | ~56 B | Per investor | Already persistent. |
| `FundingToken` | Instance | ~56 B | Frequent | Keep in instance. |
| `Treasury` | Instance | ~56 B | Frequent | Keep in instance. |
| `RegistryRef` | Instance | ~56 B | Rare | Keep in instance; small. |
| `YieldTierTable` | Instance | ~100–400 B | Frequent | Keep in instance; bounded table. |
| `FundingCloseSnapshot` | Instance | ~40 B | Frequent | Keep in instance. |
| `InvestorEffectiveYield` | Persistent | ~56 B | Per investor | Already persistent. |
| `InvestorClaimNotBefore` | Persistent | ~56 B | Per investor | Already persistent. |
| `MinContributionFloor` | Instance | ~16 B | Frequent | Keep in instance. |
| `MaxUniqueInvestorsCap` | Instance | ~4 B | Frequent | Keep in instance. |
| `MaxPerInvestorCap` | Instance | ~16 B | Frequent | Keep in instance. |
| `PendingAdmin` | Instance | ~56 B | Rare | Keep in instance; small. |
| `UniqueFunderCount` | Instance | ~4 B | Frequent | Keep in instance. |
| `PrimaryAttestationHash` | Instance | ~32 B | Frequent | Keep in instance; single digest. |
| `AttestationAppendLog` | Instance | **Unbounded** (grows per append) | Rare | **Move to persistent.** Grows without bound; loading it on every call inflates instance cost. |
| `AttestationLogEntry` | Instance | ~64–96 B per entry | Rare | **Move to persistent.** Only read when auditing the log. |
| `AttestationTimestamp` | Instance | ~8 B per entry | Rare | **Move to persistent.** Append-only audit metadata. |
| `AttestationTag` | Instance | ~32 B per entry | Rare | **Move to persistent.** Append-only audit metadata. |
| `AttestationRevoked` | Instance | ~4 B per entry | Rare | **Move to persistent.** Revocation flags are audit-only. |
| `VersionHistory` | Instance | **Unbounded** (grows per upgrade) | Rare | **Move to persistent.** History grows monotonically; not needed on hot paths. |
| `InvestorFundingHistory` | Instance | **Unbounded** (grows per investor) | Rare | **Move to persistent.** Per-investor history scales with investor count. |
| `AllowlistActive` | Instance | ~1 B | Frequent | Keep in instance. |
| `InvestorAllowlisted` | Instance | ~56 B | Per investor | Consider persistent if allowlist is large; currently instance. |
| `InvestorRefunded` | Instance | ~56 B | Per investor | Consider persistent if investor count is large; currently instance. |
| `DistributedPrincipal` | Instance | ~16 B | Frequent | Keep in instance. |
| `FundingDeadline` | Instance | ~8 B | Frequent | Keep in instance. |
| `YieldSlippageThreshold` | Instance | ~16 B | Frequent | Keep in instance. |

### Recommendations

1. **Attestation append log family** (`AttestationAppendLog`,
   `AttestationLogEntry`, `AttestationTimestamp`, `AttestationTag`,
   `AttestationRevoked`): relocate to **persistent** storage. These keys are
   append-only, grow without bound, and are only read during audit/verification
   flows — never on the hot funding/settlement path.
2. **`VersionHistory`**: relocate to **persistent** storage. It grows with each
   upgrade and is only consulted for migration/diagnostics.
3. **`InvestorFundingHistory`**: relocate to **persistent** storage. It scales
   with the number of investors and is read only for per-investor reporting.
4. **`InvestorAllowlisted` / `InvestorRefunded`**: monitor. If the investor set
   grows large, these per-address instance entries should also move to
   persistent storage; for small allowlists the current placement is fine.
5. All remaining keys are small, bounded, and/or read on hot paths — keep them
   in instance storage.

> **Out of scope:** this issue only publishes the audit. Any actual migration
> of the keys above must be handled in a follow-up issue with an explicit
> `migrate()` path.

## Contract Types

### InvoiceEscrow (State Root)
| Field | Type | Purpose |
|-------|------|---------|
| `invoice_id` | `Symbol` | State tracking |
| `admin` | `Address` | State tracking |
| `sme_address` | `Address` | State tracking |
| `amount` | `i128` | State tracking |
| `funding_target` | `i128` | State tracking |
| `funded_amount` | `i128` | State tracking |
| `yield_bps` | `i64` | State tracking |
| `maturity` | `u64` | State tracking |
| `status` | `u32` | State tracking |


### SmeCollateralCommitment
| Field | Type |
|-------|------|
| `asset` | `Symbol` |
| `amount` | `i128` |
| `recorded_at` | `u64` |

### YieldTier
| Field | Type |
|-------|------|
| `min_lock_secs` | `u64` |
| `yield_bps` | `i64` |

### FundingCloseSnapshot
| Field | Type |
|-------|------|
| `total_principal` | `i128` |
| `funding_target` | `i128` |
| `closed_at_ledger_timestamp` | `u64` |
| `closed_at_ledger_sequence` | `u32` |

### EscrowSummary
| Field | Type |
|-------|------|
| `escrow` | `InvoiceEscrow` |
| `has_maturity_lock` | `bool` |
| `legal_hold` | `bool` |
| `funding_close_snapshot` | `EscrowCloseSnapshot` |
| `unique_funder_count` | `u32` |
| `is_allowlist_active` | `bool` |
| `schema_version` | `u32` |
| `sme_collateral_commitment` | `CollateralCommitmentSnapshot` |
| `has_primary_attestation` | `bool` |
| `attestation_log_length` | `u32` |

### ErrorDiagnostic
| Field | Type |
|-------|------|
| `error_code` | `u32` |
| `message` | `String` |
| `recovery_action` | `String` |
| `context` | `Option<String>` |


## Contract Entrypoints

| Entrypoint | Auth Required | Purpose |
|------------|---------------|---------|
| `new()` | — | Additional context (e.g., timestamp, block number,... |
| `with_context()` | — | Create a diagnostic with additional context inform... |
| `init()` | — | [`validate_invoice_id_string`]). /// # Errors Emit... |
| `get_funding_token()` | — | Returns the SEP-41 funding token bound at [`Liquif... |
| `get_treasury()` | TREASURY | /// **Immutable:** set once at init; cannot change... |
| `get_registry_ref()` | — | No on-chain logic in this contract consults it. Ca... |
| `get_pending_admin()` | ADMIN | Returns the optional pending admin address waiting... |
| `verify_asset_custody()` | ADMIN | negative values indicate a shortfall. /// # Author... |
| `has_maturity_lock()` | SME | `Env::ledger().timestamp() >= maturity`. `false` m... |
| `sweep_terminal_dust()` | — | /// # Errors Emits typed [`EscrowError`] codes for... |
| `get_escrow()` | — | ... |
| `rotate_beneficiary()` | SME | |-----------|-------------| | Legal hold active | ... |
| `get_version()` | — | ... |
| `get_interface_version()` | — | arguments or return values. /// See `docs/escrow-i... |
| `get_funding_deadline()` | — | /// # Authorization /// None — pure read; no auth ... |
| `is_funding_expired()` | — | Get the optional funding deadline (ledger timestam... |
| `get_legal_hold()` | — | Whether a compliance/legal hold is active (default... |
| `get_legal_hold_clear_delay()` | — | Configured minimum delay between [`LiquifactEscrow... |
| `get_legal_hold_clearable_at()` | — | Reserved minimum ledger timestamp at which a pendi... |
| `get_min_contribution_floor()` | INVESTOR | /// **Ceilings:** [`InvoiceEscrow::funding_target`... |
| `get_max_unique_investors_cap()` | ADMIN | Optional cap on **distinct** investor addresses (`... |
| `get_max_per_investor_cap()` | INVESTOR | Optional cap on total principal for a single inves... |
| `get_unique_funder_count()` | — | Distinct funders counted so far (each address coun... |
| `get_escrow_summary()` | — | Bundles multiple read-only values to return a comp... |
| `bind_primary_attestation_hash()` | — | first wins; observers must read on-chain state (or... |
| `get_primary_attestation_hash()` | — | ... |
| `append_attestation_digest()` | — | or incremental attestation updates. Does not repla... |
| `get_attestation_append_log()` | — | ... |
| `get_contribution()` | INVESTOR | Public API: contribution recorded for `investor` (... |
| `get_funding_close_snapshot()` | — | /// The snapshot is write-once. It records the ful... |
| `get_investor_yield_bps()` | INVESTOR | calls add principal at this rate. Defaults to [`In... |
| `get_investor_claim_not_before()` | INVESTOR | Earliest ledger timestamp for [`LiquifactEscrow::c... |
| `get_yield_slippage_threshold()` | INVESTOR | Returns `0` if slippage detection is disabled (no ... |
| `get_investor_yield_slippage()` | INVESTOR | - `actual_yield_bps`: investor's effective yield (... |
| `get_sme_collateral_commitment()` | SME | Retrieve the currently recorded SME collateral com... |
| `revoke_attestation_digest()` | — | Returns `None` if no commitment has been recorded ... |
| `is_attestation_revoked()` | — | ... |
| `is_investor_claimed()` | — | ... |
| `record_sme_collateral_commitment()` | SME | - [`EscrowError::CollateralAssetEmpty`] if `asset`... |
| `set_legal_hold()` | ADMIN | **Governance posture:** production `admin` must be... |
| `request_clear_legal_hold()` | — | returned ledger timestamp is reached. /// # Errors... |
| `set_allowlist_active()` | INVESTOR | Enable or disable the investor allowlist. When ena... |
| `is_allowlist_active()` | — | ... |
| `set_investor_allowlisted()` | INVESTOR | Add or remove an investor from the allowlist.... |
| `set_investors_allowlisted()` | INVESTOR | `set_investor_allowlisted` individually for each e... |
| `is_investor_allowlisted()` | — | ... |
| `clear_legal_hold()` | — | Convenience alias for [`LiquifactEscrow::set_legal... |
| `update_funding_target()` | — | Convenience alias for [`LiquifactEscrow::set_legal... |
| `lower_max_unique_investors()` | INVESTOR | /// # Panics - If the escrow is not open. - If no ... |
| `migrate()` | — | | Any `from_version < SCHEMA_VERSION` (all paths) ... |
| `upgrade()` | — | new WASM would corrupt stored data. Operators must... |
| `fund()` | — | /// # Errors Emits typed [`EscrowError`] codes for... |
| `fund_with_commitment()` | INVESTOR | from the same investor must use [`LiquifactEscrow:... |
| `partial_settle()` | ADMIN | Closes funding early for an under-funded invoice, ... |
| `settle()` | — | ... |
| `withdraw()` | — | 7. Event emission. /// # Errors - [`EscrowError::L... |
| `claim_investor_payout()` | INVESTOR | 6. Idempotent early-return on `InvestorClaimed`. 7... |
| `compute_investor_payout()` | — | All multiplications use [`i128::checked_mul`] and ... |
| `update_maturity()` | — | ... |
| `bump_ttl()` | — | ... |
| `propose_admin()` | ADMIN | /// Requires current admin authorization. The dest... |
| `accept_admin()` | ADMIN | /// The address stored in [`DataKey::PendingAdmin`... |
| `transfer_admin()` | ADMIN | This function now only proposes `new_admin` by del... |
| `cancel_funding()` | INVESTOR | After cancellation, investors may recover their pr... |
| `refund()` | INVESTOR | /// # Errors Emits typed [`EscrowError`] codes whe... |
| `is_investor_refunded()` | INVESTOR | Whether an investor has already received a refund ... |
| `get_distributed_principal()` | INVESTOR | Total principal already returned to investors via ... |