use super::*;

/// Full state of an invoice escrow persisted in contract storage (`DataKey::Escrow`).
#[contracttype]
#[derive(Debug, PartialEq)]
/// Full escrow snapshot persisted at [`DataKey::Escrow`].
///
/// Derive rationale:
/// - `Debug`: improves failure diagnostics in tests.
/// - `PartialEq`: allows exact state assertions in tests.
///
/// `Clone` is intentionally omitted to avoid accidental full-state copies.
pub struct InvoiceEscrow {
    pub invoice_id: Symbol,
    pub admin: Address,
    pub sme_address: Address,
    pub amount: i128,
    pub funding_target: i128,
    pub funded_amount: i128,
    pub yield_bps: i64,
    pub maturity: u32,
    /// 0 = open, 1 = funded, 2 = settled, 3 = withdrawn (SME pulled liquidity), 4 = cancelled (admin-gated; investors may refund), 5 = archived (admin-gated; read-only terminal)
    pub status: u32,
    pub guardian: Option<Address>,
}

/// SME-reported collateral metadata for off-chain risk review.
///
/// **Record-only:** this struct is stored for transparency and indexing. It does **not**
/// custody, escrow, transfer, freeze, reserve, or verify assets. It also does not alter funding,
/// settlement, SME withdrawal, investor-claim, compliance hold, or treasury-sweep behavior.
/// Future versions that enforce asset movement or custody must introduce explicit APIs and must
/// not treat historical records from this type as proof of locked assets.
///
/// Complete escrow state snapshot for archival, analysis, compliance, and audit reporting.
/// Returned by [`LiquifactEscrow::export_escrow_snapshot`]. See that entrypoint's rustdoc for
/// what is and isn't included.
#[contracttype]
#[derive(Debug, PartialEq)]
pub struct EscrowSnapshot {
    pub escrow: InvoiceEscrow,
    pub schema_version: u32,
    pub legal_hold: bool,
    pub funding_close_snapshot: EscrowCloseSnapshot,
    pub unique_funder_count: u32,
    pub is_allowlist_active: bool,
    pub sme_collateral_commitment: CollateralCommitmentSnapshot,
    pub primary_attestation_hash: Option<BytesN<32>>,
    pub attestation_log_length: u32,
    pub sanctions_provider: Option<Address>,
    pub funding_token: Address,
    pub treasury: Address,
    pub registry: Option<Address>,
    pub version_history: Vec<(u32, u64)>,
}

/// Compile-time build provenance embedded in the WASM artifact by `escrow/build.rs`.
/// Returned by [`LiquifactEscrow::get_build_metadata`]. See that entrypoint's rustdoc.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct BuildMetadata {
    pub schema_version: u32,
    pub interface_version: u32,
    pub git_commit: String,
    pub git_full_commit: String,
    pub build_timestamp: String,
    pub pkg_version: String,
    pub rust_version: String,
}

/// Read-only dashboard metrics for this escrow, returned by
/// [`LiquifactEscrow::get_escrow_health_metrics`].
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct EscrowHealthMetrics {
    /// `(funded_amount * 100) / funding_target`, floored, capped at `100` when overfunded.
    pub funding_progress_percent: u32,
    /// Days until `maturity` (negative once maturity has passed). `0` when no maturity is set.
    pub days_to_maturity: i64,
    pub unique_investor_count: u32,
    /// `funded_amount / unique_investor_count`, or `0` when there are no investors yet.
    pub average_contribution_size: i128,
    /// Flat coupon on the whole `funded_amount`; does not account for tiered per-investor yield.
    pub estimated_yield_payout: i128,
}

/// Investor capacity status for this escrow.
///
/// Provides a self-explanatory answer to "can more investors contribute?"
/// in a single read-only entrypoint, avoiding error-prone arithmetic on the client side.
///
/// Returned by [`LiquifactEscrow::get_investor_cap_status`].
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct InvestorCapStatus {
    /// Maximum number of distinct investors allowed (u32::MAX if unlimited).
    pub max: u32,
    /// Current number of distinct investors that have contributed.
    pub current: u32,
    /// Remaining capacity for new investors (max - current).
    pub remaining: u32,
    /// True when current == max (escrow is at capacity).
    pub is_full: bool,
}

/// Discovery metadata for this escrow, suitable for registry contracts and off-chain indexers.
/// Returned by [`LiquifactEscrow::get_registry_listing`].
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RegistryListing {
    pub escrow_address: Address,
    pub invoice_id: Symbol,
    pub sme_address: Address,
    /// `0` for escrows that predate [`DataKey::CreatedAt`].
    pub created_at: u64,
    pub status: u32,
    pub funding_target: i128,
}

/// Optional filters for [`LiquifactEscrow::query_attestation_logs`]. All fields are additive
/// (AND-combined); leave a field `None` to skip that filter. An entirely empty filter matches
/// every entry.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct AuditLogFilter {
    /// Only entries with `timestamp >= after_timestamp` (inclusive).
    pub after_timestamp: Option<u64>,
    /// Only entries whose tag exactly equals this value (see the entrypoint's rustdoc).
    pub tag_prefix: Option<Symbol>,
}

/// One entry in the bounded attestation append log, as returned by
/// [`LiquifactEscrow::query_attestation_logs`].
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct AttestationLogEntry {
    pub digest: BytesN<32>,
    /// Position of this entry in the append log (`0`-based).
    pub index: u32,
    /// Ledger timestamp recorded at [`LiquifactEscrow::append_attestation_digest`] time.
    pub timestamp: u64,
    /// Optional caller-supplied tag; empty [`Symbol`] when none was provided.
    pub tag: Symbol,
    /// Whether this entry has since been revoked via [`LiquifactEscrow::revoke_attestation_digest`].
    pub revoked: bool,
}

/// A paginated page of [`AttestationLogEntry`] results from
/// [`LiquifactEscrow::query_attestation_logs`].
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct AuditLogPage {
    pub entries: Vec<AttestationLogEntry>,
    /// Total number of entries matching the filter, independent of pagination `limit`/`offset`.
    pub total_count: u32,
}

/// Flags for each logical invariant checked by [`LiquifactEscrow::detect_state_inconsistencies`].
/// A `true` value indicates the corresponding inconsistency was detected; an all-`false` report
/// signals valid state. See the entrypoint's rustdoc for what each flag means.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct StateInconsistencyReport {
    pub funded_exceeds_target_stale: bool,
    pub funded_positive_status_open: bool,
    pub funded_zero_status_advanced: bool,
    pub funders_exist_status_open: bool,
    pub no_funders_advanced_status: bool,
    pub snapshot_exists_not_funded: bool,
    pub snapshot_missing_post_funded: bool,
    pub settled_before_maturity_lock: bool,
    pub invalid_funding_amounts: bool,
    pub invalid_status_value: bool,
}

/// Snapshot of compliance-relevant configuration and state for this escrow, embedded in
/// [`ComplianceReportGenerated`].
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ComplianceReport {
    pub legal_hold_active: bool,
    pub is_allowlist_active: bool,
    pub kyc_provider: Option<Address>,
    pub sanctions_provider: Option<Address>,
    pub unique_investor_count: u32,
    pub generated_at_ledger_timestamp: u64,
}

/// # Fields
/// - `asset`: The off-chain asset symbol (cannot be empty).
/// - `amount`: The reported collateral amount (must be positive).
/// - `recorded_at`: The Soroban ledger timestamp when this record was **first** written. Immutable
///   after the initial call; subsequent updates preserve this value.
/// - `updated_at`: The Soroban ledger timestamp of the most recent write (including the initial
///   write). Equals `recorded_at` when no update has occurred.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
/// SME collateral commitment metadata (record-only).
///
/// Derive rationale:
/// - `Clone`: required for `Option<SmeCollateralCommitment>` used in `EscrowSummary`.
/// - `Debug`: improves failure diagnostics in tests.
/// - `PartialEq`: allows deterministic assertion of stored/read values.
pub struct SmeCollateralCommitment {
    pub asset: Symbol,
    pub amount: i128,
    pub recorded_at: u32,
}

/// Incremental state change record for delta-encoded snapshots.
/// Stores only the fields that changed from the previous state, enabling storage optimization.
///
/// **Immutable** once written; delta chains form an audit trail of state transitions.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotDelta {
    /// Unique ID of this delta (monotonically increasing).
    pub delta_id: u32,
    /// Ledger timestamp when this delta was recorded.
    pub recorded_at: u32,
    /// Previous delta ID this one is based on (0 for baseline/first delta).
    pub based_on_delta_id: u32,
    /// Change in funded amount (signed; may be negative for reversals).
    pub funded_amount_delta: i128,
    /// New maturity value (0 if unchanged).
    pub maturity: u32,
    /// New status (255 if unchanged).
    pub status: u32,
    /// New admin address (None if unchanged).
    pub admin: Option<Address>,
    /// New SME/beneficiary address (None if unchanged).
    pub sme_address: Option<Address>,
}

/// One step in an optional tier ladder: investors who commit to at least `min_lock_secs` (on first
/// deposit via [`LiquifactEscrow::fund_with_commitment`]) may receive `yield_bps` for pro-rata /
/// off-chain coupon math. **Immutable** after `init`: the table is fixed for the escrow instance.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct YieldTier {
    pub min_lock_secs: u32,
    pub yield_bps: i64,
}

/// Captured exactly once at the first ledger transition to **funded** so settlement and claims can
/// use a stable total principal and target. If the threshold-crossing deposit overshoots
/// [`InvoiceEscrow::funding_target`], [`FundingCloseSnapshot::total_principal`] records the full
/// credited [`InvoiceEscrow::funded_amount`] at close and becomes the pro-rata denominator.
/// **Immutable** once written.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct FundingCloseSnapshot {
    /// Sum of principal credited when the invoice became funded (`funded_amount` at close),
    /// including over-funding past target.
    pub total_principal: i128,
    pub funding_target: i128,
    pub closed_at_ledger_timestamp: u32,
    pub closed_at_ledger_sequence: u32,
}

/// Custom option-like enum to represent the captured funding close snapshot.
/// Models standard option semantics as a contracttype to avoid standard library
/// blanket trait limitations in Soroban SDK testutils.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum EscrowCloseSnapshot {
    None,
    Some(FundingCloseSnapshot),
}

/// Custom option-like enum to represent the SME collateral commitment.
/// Models standard option semantics as a contracttype to avoid standard library
/// blanket trait limitations in Soroban SDK testutils.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum CollateralCommitmentSnapshot {
    None,
    Some(SmeCollateralCommitment),
}

/// Cached token metadata to reduce external calls during fund operations.
///
/// **Purpose:** Caches token contract metadata (decimals, name, symbol) at escrow
/// initialization time to avoid repeated cross-contract calls on every fund operation.
/// This is a read-only cache that is only updated via explicit revalidation entrypoint.
///
/// **Cache Policy:**
/// - Written once at initialization by [`LiquifactEscrow::init`]
/// - Updated only via [`LiquifactEscrow::revalidate_token_cache`] (admin-only)
/// - Read by fund operations to validate amounts and compute scaled values
/// - Immutable for normal operation (no mutations except explicit revalidation)
///
/// **Fields:**
/// - `decimals`: Token decimal precision (typically 7 for Stellar)
/// - `cached_at_ledger_timestamp`: Timestamp when cache was last written
/// - `cached_at_ledger_sequence`: Ledger sequence when cache was last written
///
/// **Versioning:** Includes ledger time/sequence to allow detecting stale caches
/// (e.g., if token contract upgrade changes decimal places). Exact staleness
/// detection is left to governance processes.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct TokenMetadataCache {
    /// Token decimal places (e.g., 7 for Stellar USDC)
    pub decimals: u32,
    /// Ledger timestamp when cache was written (for staleness detection)
    pub cached_at_ledger_timestamp: u32,
    /// Ledger sequence when cache was written (for staleness detection)
    pub cached_at_ledger_sequence: u32,
}

/// Comprehensive summary of the escrow contract state.
/// Bundles multiple read-only values to allow a single host invocation
/// for off-chain indexers and client rendering.
#[contracttype]
#[derive(Debug, PartialEq)]
pub struct EscrowSummary {
    /// Full escrow snapshot.
    pub escrow: InvoiceEscrow,
    /// True when `escrow.maturity > 0`; false means settlement has no maturity time lock.
    pub has_maturity_lock: bool,
    /// Active legal or compliance hold flag.
    pub legal_hold: bool,
    /// The captured funding close snapshot (Option).
    pub funding_close_snapshot: EscrowCloseSnapshot,
    /// Unique investors count who funded the escrow.
    pub unique_funder_count: u32,
    /// Whether the investor allowlist is active.
    pub is_allowlist_active: bool,
    /// Persisted schema version of the contract data.
    pub schema_version: u32,
    /// SME collateral commitment metadata (None when never recorded).
    pub sme_collateral_commitment: CollateralCommitmentSnapshot,
    /// Whether a primary attestation hash has been bound.
    pub has_primary_attestation: bool,
    /// Number of entries in the attestation append log.
    pub attestation_log_length: u32,
    /// Optional yield-bearing token address (None when unconfigured).
    pub yield_token: Option<Address>,
    /// Optional oracle contract address (None when unconfigured).
    pub oracle_contract: Option<Address>,
    /// Optional NFT contract address (None when unconfigured).
    pub nft_contract: Option<Address>,
    /// Settlement NFT metadata if minted (None when no NFT contract is configured or not yet settled).
    pub settlement_nft: Option<SettlementNftMetadata>,
}

/// Automatic yield distribution snapshot computed at settlement time.
///
/// When [`LiquifactEscrow::settle`] runs with `yield_auto_distribution_enabled = true`,
/// it pre-computes each investor's yield share and stores it in **persistent** storage
/// under [`DataKey::YieldDistributionShare`] for that address. This enables batch
/// claim operations without per-investor yield recalculation.
///
/// **Immutability:** Once computed, yield shares are immutable for this settlement.
/// Subsequent [[`claim_investor_payout`] calls read the pre-computed value if available.
///
/// **Backwards compatibility:** If `yield_auto_distribution_enabled = false`, this
/// structure is never written, and claims compute yields on-demand as before.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct YieldDistributionSnapshot {
    /// Per-investor yield amount (principal + share of coupon).
    /// Computed as: `contribution_share * (settled_amount + net_coupon) / settled_amount`.
    pub payout_amount: i128,
    /// Ledger timestamp when this distribution was captured (at settlement time).
    pub captured_at_ledger_timestamp: u64,
    /// Ledger sequence when this distribution was captured (at settlement time).
    pub captured_at_ledger_sequence: u32,
}

/// Metadata for an NFT minted against [`DataKey::NftContract`] to represent this escrow's
/// settlement (e.g. a receipt/certificate NFT). No minting logic is currently wired up in this
/// contract; the type exists so [`DataKey::SettlementNft`] has a concrete shape once it is.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SettlementNftMetadata {
    pub token_id: u32,
    pub minted_at_ledger_timestamp: u64,
}

/// Custom option-like enum to represent the settlement NFT metadata.
/// Models standard option semantics as a contracttype.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum SettlementNftSnapshot {
    None,
    Some(SettlementNftMetadata),
}

/// State of a dispute pause on an escrow instance.
///
/// Dispute pause is **separate** from legal hold and is triggered by admin for dispute
/// resolution (e.g., invoice validity challenge). It auto-expires after the configured
/// duration or is manually resumed.
///
/// # Fields
/// - `ticket_id`: Identifier or reference to the support/dispute ticket (for audit trail).
/// - `paused_at_ledger_timestamp`: Soroban ledger timestamp when pause was activated.
/// - `expires_at_ledger_timestamp`: Ledger timestamp after which the pause auto-expires.
///   Once ledger time reaches or exceeds this value, the escrow is unpaused.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DisputePauseState {
    pub ticket_id: String,
    pub paused_at_ledger_timestamp: u32,
    pub expires_at_ledger_timestamp: u32,
}

// --- Events ---

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DisputeResumedBy {
    Manual,
    AutoExpiry,
}

/// Audit log entry for reinvestment elections and events.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ReinvestmentAuditEntry {
    /// Investor address that made the reinvestment election
    pub investor: Address,
    /// Current escrow's invoice ID
    pub invoice_id: Symbol,
    /// Target escrow contract for reinvestment (None = same escrow)
    pub target_escrow: Option<Address>,
    /// Yield amount being reinvested into principal
    pub reinvested_amount: i128,
    /// Ledger timestamp when reinvestment was elected
    pub elected_at_ledger_timestamp: u64,
    /// Ledger sequence when reinvestment was elected
    pub elected_at_ledger_sequence: u32,
}

/// One checkpoint entry in an investor's [`DataKey::InvestorFundingHistory`] audit trail.
///
/// Recorded at each contribution ([`LiquifactEscrow::fund`] /
/// [`LiquifactEscrow::fund_with_commitment`]) and at settlement claim
/// ([`LiquifactEscrow::claim_investor_payout`]). `checkpoint` distinguishes the kind of
/// event: `"deposit"`, `"funding_close"` (the deposit that closed funding), or `"settlement"`.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct InvestorFundingRecord {
    /// Kind of checkpoint: `"deposit"`, `"funding_close"`, or `"settlement"`.
    pub checkpoint: Symbol,
    /// Ledger timestamp when this checkpoint was recorded.
    pub timestamp: u64,
    /// Amount associated with this checkpoint (deposit amount, or payout amount at settlement).
    pub amount: i128,
    /// Investor's cumulative principal contribution as of this checkpoint.
    pub cumulative_total: i128,
}

/// Tiered admin privilege level stored in [`DataKey::AdminRoles`].
///
/// ## Role capabilities matrix
///
/// | Role | Full-admin ops (via `load_escrow_require_admin`) | Settlement-tier ops | Audit / read-only |
/// |------|:--:|:--:|:--:|
/// | [`AdminRole::Full`] | ✅ | ✅ | ✅ |
/// | [`AdminRole::SettlementOnly`] | ❌ | ✅ | ✅ |
/// | [`AdminRole::AuditOnly`] | ❌ | ❌ | ✅ |
///
/// All public getters are unauthenticated reads and available to every role (and to callers
/// with no role at all). [`InvoiceEscrow::admin`] is always implicitly [`AdminRole::Full`]
/// unless explicitly demoted via [`LiquifactEscrow::set_admin_roles`].
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminRole {
    /// Read-only / record-keeping tier: no mutating admin privileges.
    AuditOnly,
    /// May authorize settlement-tier operations (e.g. [`LiquifactEscrow::partial_settle`]) in
    /// addition to [`AdminRole::AuditOnly`] capabilities.
    SettlementOnly,
    /// All admin capabilities, including operations gated by
    /// [`LiquifactEscrow::load_escrow_require_admin`] (legal hold, allowlist, caps, migrations, …).
    Full,
}

impl AdminRole {
    /// Privilege ordering for [`AdminRole`] comparisons: higher ⇒ more capable.
    pub(crate) fn level(&self) -> u32 {
        match self {
            AdminRole::AuditOnly => 1,
            AdminRole::SettlementOnly => 2,
            AdminRole::Full => 3,
        }
    }
}

/// Multisig policy gating critical operations, configured by
/// [`LiquifactEscrow::init_multisig_policy`] and stored at [`DataKey::MultisigPolicy`].
///
/// `operations` names the operation tags covered by this policy (see
/// [`LiquifactEscrow::set_legal_hold_multisig`] / [`LiquifactEscrow::release_large_claim_multisig`]
/// for the tags each entrypoint checks). An operation **not** listed here falls back to requiring
/// only the single [`InvoiceEscrow::admin`] signature.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct MultisigPolicy {
    /// Operation tags this policy covers (e.g. `"legal_hold"`, `"large_claim"`).
    pub operations: Vec<Symbol>,
    /// Addresses eligible to co-sign a covered operation.
    pub signers: Vec<Address>,
    /// Minimum number of distinct `signers` members that must authorize a covered call.
    pub threshold: u32,
}

/// Diagnostic information for contract errors, emitted alongside error returns.
///
/// SDKs and integrators parse this struct to provide user-friendly error messages,
/// recovery suggestions, and contextual information (e.g., "can claim in X blocks").
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorDiagnostic {
    /// Error code (matches [`EscrowError`] discriminant).
    pub error_code: u32,
    /// Human-readable error message.
    pub message: String,
    /// Suggested recovery action or next steps.
    pub recovery_action: String,
    /// Additional context (e.g., timestamp, block number, current value).
    pub context: Option<String>,
}

impl ErrorDiagnostic {
    /// Builds an [`ErrorDiagnostic`] with a populated `context` field, converting each `&str`
    /// argument into a Soroban [`String`] bound to `env`.
    pub(crate) fn with_context(
        env: &Env,
        error_code: u32,
        message: &str,
        recovery_action: &str,
        context: &str,
    ) -> Self {
        ErrorDiagnostic {
            error_code,
            message: String::from_str(env, message),
            recovery_action: String::from_str(env, recovery_action),
            context: Some(String::from_str(env, context)),
        }
    }
}
