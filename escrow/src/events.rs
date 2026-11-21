use super::*;

#[contractevent]
pub struct EscrowInitialized {
    #[topic]
    pub name: Symbol,
    pub escrow: InvoiceEscrow,
    /// Bound funding token; equals [`DataKey::FundingToken`].
    pub funding_token: Address,
    /// Bound treasury; equals [`DataKey::Treasury`].
    pub treasury: Address,
    /// Optional registry hint; equals [`DataKey::RegistryRef`] (`None` when unset).
    pub registry: Option<Address>,
    /// False when `escrow.maturity == 0`, which means `settle` has no maturity time lock.
    pub has_maturity_lock: bool,
    /// Optional yield-bearing token; equals [`DataKey::YieldToken`] (`None` when unset).
    pub yield_token: Option<Address>,
    /// Optional oracle contract; equals [`DataKey::OracleContract`] (`None` when unset).
    pub oracle_contract: Option<Address>,
    /// Optional NFT contract; equals [`DataKey::NftContract`] (`None` when unset).
    pub nft_contract: Option<Address>,
}

#[contractevent]
pub struct RegistryRegistrationAttempted {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub registry: Option<Address>,
    pub successful: bool,
}

#[contractevent]
pub struct MaxUniqueInvestorsCapLowered {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub old_cap: u32,
    pub new_cap: u32,
}

#[contractevent]
pub struct EscrowFunded {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    #[topic]
    pub investor: Address,
    pub amount: i128,
    pub funded_amount: i128,
    pub status: u32,
    /// Investor-specific effective yield (bps) after this fund; see [`DataKey::InvestorEffectiveYield`].
    pub investor_effective_yield_bps: i64,
    /// The `min_lock_secs` of the matched [`YieldTier`] (0 when base yield applies — no tier,
    /// no lock commitment, or simple fund). See [`LiquifactEscrow::effective_yield_for_commitment`].
    pub tier_lock_secs: u32,
}

/// Versioned counterpart to [`EscrowFunded`] carrying `actor` + `timestamp`, mirroring the
/// [`AdminChanged`] pattern used for other lifecycle events.
#[contractevent]
pub struct FundReceived {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    #[topic]
    pub actor: Address,
    pub timestamp: u64,
    pub amount: i128,
    pub funded_amount: i128,
    pub status: u32,
    pub investor_effective_yield_bps: i64,
    pub tier_lock_secs: u64,
}

/// Emitted once by [`LiquifactEscrow::batch_fund`] after all entries are successfully processed.
///
/// Provides a roll-up view of the entire batch: total principal credited and the number of
/// distinct investor entries processed. Indexers and dashboards can use this event to
/// reconcile batch submissions without scanning every individual [`EscrowFunded`] event.
///
/// Per-entry [`EscrowFunded`] and [`FundReceived`] events are still emitted by the underlying
/// [`fund_impl`] for full per-investor audit coverage.
#[contractevent]
pub struct BatchFundCompleted {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// Total principal credited across all entries in this batch (sum of all amounts).
    pub total_funded_amount: i128,
    /// Number of investor entries processed in this batch call.
    pub investor_count: u32,
    /// Final `funded_amount` stored in the escrow after the batch completes.
    pub escrow_funded_amount: i128,
    /// Final escrow status after the batch completes (`0` = open, `1` = funded).
    pub status: u32,
}

/// Emitted by [`LiquifactEscrow::archive_escrow`] when a terminal escrow (settled, withdrawn,
/// or cancelled) transitions to the read-only archived status (`5`).
#[contractevent]
pub struct EscrowArchived {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// The terminal status (`2`, `3`, or `4`) the escrow was in immediately before archiving.
    pub prior_status: u32,
    pub archived_at_ledger_timestamp: u64,
}

/// Emitted by [`LiquifactEscrow::rotate_beneficiary`] when the SME (beneficiary)
/// address is changed, carrying both the prior and new addresses for auditing.
#[contractevent]
pub struct BeneficiaryRotated {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub prior_sme: Address,
    pub new_sme: Address,
}

#[contractevent]
pub struct EscrowPartialSettle {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub funded_amount: i128,
}

#[contractevent]
pub struct EscrowSettled {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub funded_amount: i128,
    pub yield_bps: i64,
    pub maturity: u32,
    /// Ledger timestamp at which the settlement occurred.
    pub settled_at_ledger_timestamp: u32,
}

#[contractevent]
pub struct EscrowPartiallySettled {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub funded_amount: i128,
    pub settled_amount: i128,
    pub yield_bps: i64,
    pub maturity: u32,
    /// Ledger timestamp at which the partial settlement occurred.
    pub settled_at_ledger_timestamp: u32,
}

#[contractevent]
pub struct MaturityUpdatedEvent {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub old_maturity: u32,
    pub new_maturity: u32,
}

#[contractevent]
pub struct AdminTransferredEvent {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub new_admin: Address,
}

#[contractevent]
pub struct AdminProposedEvent {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub current_admin: Address,
    pub pending_admin: Address,
}

/// Emitted by [`LiquifactEscrow::init`] when the supplied `admin` address resolves to a
/// deployed contract rather than a regular account (keypair).
///
/// A contract admin is not inherently unsafe — it is the standard pattern for multisig and
/// DAO governance — but it widens the attack surface compared to a simple account. Indexers
/// and monitoring tools should surface this event to operators for manual review.
///
/// If the caller passed `reject_contract_admin = Some(true)`, [`LiquifactEscrow::init`] fails
/// with [`EscrowError::ContractAdminRejected`] (code 181) **instead** of emitting this event.
#[contractevent]
pub struct AdminIsContractWarning {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// The contract address that was supplied as `admin`.
    pub admin: Address,
}

#[contractevent]
pub struct FundingTargetUpdated {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub old_target: i128,
    pub new_target: i128,
}

#[contractevent]
pub struct LegalHoldChanged {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// `1` = hold enabled, `0` = cleared.
    pub active: u32,
    /// Human-readable reason for the hold (max 256 bytes). Empty when clearing.
    pub reason: String,
}

/// Emitted by [`LiquifactEscrow::set_legal_hold_multisig`] instead of [`LegalHoldChanged`].
#[contractevent]
pub struct LegalHoldChangedMultisig {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// `1` = hold enabled, `0` = cleared.
    pub active: u32,
    /// Human-readable reason for the hold. Empty when clearing.
    pub reason: String,
    /// Number of distinct co-signers that authorized this call.
    pub signer_count: u32,
}

#[contractevent]
pub struct LegalHoldClearRequested {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// Inclusive ledger timestamp when clearing may occur.
    pub clearable_at: u32,
}

#[contractevent]
pub struct LegalHoldProposed {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub admin: Address,
    pub expires_at: u64,
}

/// Emitted when a dispute pause is activated on an escrow.
///
/// Dispute pause is separate from legal hold and is used for temporary resolution
/// of invoice disputes (e.g., validity challenges). The escrow is frozen until either
/// the auto-expiration timestamp is reached or an admin manually resumes it.
///
/// # Fields
/// - `name`: Hardcoded symbol (e.g., `dispute_pause` or `disp_pause`).
/// - `invoice_id`: Symbol representation of the invoice.
/// - `ticket_id`: Support/dispute ticket reference for audit trail.
/// - `action`: `1` = paused.
/// - `paused_at`: Ledger timestamp when pause was activated.
/// - `expires_at`: Ledger timestamp when the pause is configured to auto-expire.
#[contractevent]
pub struct DisputePausedEvt {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub ticket_id: String,
    /// `1` = paused, `0` = resumed.
    pub action: u32,
    pub paused_at: u32,
    pub expires_at: u32,
}

/// Emitted when an administrator manually resumes a dispute pause, or when an
/// expired pause is first observed by a mutating operation.
#[contractevent]
pub struct DisputeResumedEvt {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub admin: Address,
    pub resumed_by: DisputeResumedBy,
    pub ledger_timestamp: u64,
}

/// SME collateral commitment metadata recorded.
///
/// This event is emitted when [`DataKey::SmeCollateralPledge`] is written or replaced by the SME.
/// It acts as a metadata-update signal and is not proof of custody, lien, encumbrance, asset control,
/// or token movement. The event intentionally omits token contract, custodian, and transfer-receipt
/// fields so consumers do not treat it as an on-chain encumbrance.
///
/// # Fields
/// - `name`: Hardcoded `coll_rec` symbol.
/// - `invoice_id`: Symbol representation of the invoice.
/// - `amount`: Newly recorded positive collateral amount.
/// - `prior_amount`: Prior recorded collateral amount (or `0` if none existed).
#[contractevent]
pub struct CollateralRecordedEvt {
    #[topic]
    pub name: Symbol,
    /// Invoice whose SME-reported metadata was updated.
    pub invoice_id: Symbol,
    /// SME-reported amount in the off-chain asset's own units; not a locked token balance.
    pub amount: i128,
    /// Prior recorded amount, or 0 if no prior commitment existed.
    pub prior_amount: i128,
}

#[contractevent]
pub struct SmeWithdrew {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub amount: i128,
    pub recipient: Address,
}

#[contractevent]
pub struct InvestorPayoutClaimed {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub investor: Address,
    #[topic]
    pub invoice_id: Symbol,
}

/// Emitted when a batch of investor claims completes successfully.
/// Carries the count of claims processed so indexers can reconcile
/// against individual [`InvestorPayoutClaimed`] events emitted per investor.
#[contractevent]
pub struct BatchInvestorPayoutsClaimed {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// Number of investors whose claims were processed in this batch.
    pub claimed_count: u32,
}

/// Emitted when automatic yield distribution snapshot is computed at settlement time.
/// Signals that all investors' yields have been pre-calculated and stored for batch claims.
#[contractevent]
pub struct YieldDistSnapshotCreated {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// Total settled amount used for yield computation.
    pub settled_amount: i128,
    /// Number of unique investors with pre-computed yield shares.
    pub investor_count: u32,
    /// Ledger timestamp when snapshot was captured.
    pub captured_at_ledger_timestamp: u64,
}

/// Emitted when an investor's auto-distributed yield is claimed.
/// Indicates that the investor used the pre-computed yield amount from the snapshot.
#[contractevent]
pub struct AutoDistributedYieldClaimed {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub investor: Address,
    #[topic]
    pub invoice_id: Symbol,
    /// Pre-computed payout amount from the yield distribution snapshot.
    pub payout_amount: i128,
}

/// Emitted when automatic yield distribution is enabled for an escrow.
#[contractevent]
pub struct YieldAutoDistributionEnabled {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub enabled: bool,
    pub timestamp: u64,
}

/// Emitted when automatic yield distribution is disabled for an escrow.
#[contractevent]
pub struct YieldAutoDistributionDisabled {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub timestamp: u64,
}

/// Emitted by [`LiquifactEscrow::revalidate_token_cache`] after refreshing
/// [`DataKey::TokenMetadataCache`] with fresh decimals from the token contract.
#[contractevent]
pub struct TokenCacheRevalidated {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub decimals: u32,
    pub revalidated_ledger_timestamp: u64,
}

/// Emitted by [`LiquifactEscrow::notify_settlement`] after successfully invoking the
/// configured [`DataKey::SettlementNotifierContract`].
#[contractevent]
pub struct SettlementNotifierInvoked {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub notifier_contract: Address,
    pub funded_amount: i128,
    pub yield_bps: i64,
    pub settled_at_ledger_timestamp: u64,
}

/// Emitted when a funding-close Merkle root is computed and bound to
/// [`DataKey::FundingCloseMerkleRoot`] (on full or partial funding close).
#[contractevent]
pub struct FundingCloseMerkleRootBound {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub merkle_root: BytesN<32>,
}

/// Emitted by [`LiquifactEscrow::detect_state_inconsistencies`] when at least one flag in the
/// returned [`StateInconsistencyReport`] is `true`.
#[contractevent]
pub struct StateInconsistenciesDetected {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub report: StateInconsistencyReport,
}

#[contractevent]
pub struct YieldSlippageWarning {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub investor: Address,
    #[topic]
    pub invoice_id: Symbol,
    /// Expected yield (in basis points) based on configured `yield_bps`.
    pub expected_yield_bps: i64,
    /// Actual effective yield (in basis points) determined at settlement.
    pub actual_yield_bps: i64,
    /// Configured slippage threshold (in basis points).
    pub slippage_threshold_bps: i64,
    /// Computed deviation (absolute difference) in basis points.
    pub deviation_bps: i64,
}

#[contractevent]
pub struct FundingCancelled {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub funded_amount: i128,
}

#[contractevent]
pub struct InvestorRefundedEvt {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub investor: Address,
    #[topic]
    pub invoice_id: Symbol,
    pub amount: i128,
}

#[contractevent]
pub struct TreasuryDustSwept {
    #[topic]
    pub name: Symbol,
    pub invoice_id: Symbol,
    pub token: Address,
    pub amount: i128,
}

#[contractevent]
pub struct AssetCustodyVerified {
    #[topic]
    pub name: Symbol,
    pub invoice_id: Symbol,
    pub recorded_funded_amount: i128,
    pub contract_balance: i128,
    pub discrepancy: i128,
}

#[contractevent]
pub struct PrimaryAttestationBound {
    #[topic]
    pub name: Symbol,
    pub invoice_id: Symbol,
    pub digest: BytesN<32>,
}

#[contractevent]
pub struct AttestationBoundEvt {
    #[topic]
    pub name: Symbol,
    pub hash: BytesN<32>,
    pub ledger_timestamp: u64,
}

#[contractevent]
pub struct AttestationDigestAppended {
    #[topic]
    pub name: Symbol,
    pub invoice_id: Symbol,
    pub index: u32,
    pub digest: BytesN<32>,
}

#[contractevent]
pub struct AttestationDigestRevoked {
    #[topic]
    pub name: Symbol,
    pub invoice_id: Symbol,
    pub index: u32,
}

#[contractevent]
pub struct AllowlistEnabledChanged {
    #[topic]
    pub name: Symbol,
    pub invoice_id: Symbol,
    /// `1` = enabled, `0` = disabled.
    pub active: u32,
}

#[contractevent]
pub struct InvestorAllowlistChanged {
    #[topic]
    pub name: Symbol,
    pub invoice_id: Symbol,
    pub investor: Address,
    /// `1` = allowed, `0` = blocked.
    pub allowed: u32,
}

/// Emitted when a compliance report is generated via
/// [`LiquifactEscrow::generate_compliance_report`].
#[contractevent]
pub struct ComplianceReportGenerated {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub report: ComplianceReport,
}

/// Emitted when a protocol fee is collected from yield and transferred to treasury.
#[contractevent]
pub struct ProtocolFeeCollected {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// Gross yield coupon before fee deduction.
    pub gross_coupon: i128,
    /// Fee percentage in basis points.
    pub fee_percentage: i64,
    /// Fee amount transferred to treasury.
    pub fee_amount: i128,
    /// Net yield after fee deduction.
    pub net_coupon: i128,
}

/// Emitted when an investor elects to reinvest their yield.
#[contractevent]
pub struct YieldReinvestmentElected {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub investor: Address,
    #[topic]
    pub invoice_id: Symbol,
    /// Target escrow address (if reinvesting to different escrow)
    pub target_escrow: Option<Address>,
}

/// Emitted when yield is automatically reinvested as principal.
#[contractevent]
pub struct YieldReinvested {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub investor: Address,
    #[topic]
    pub invoice_id: Symbol,
    /// Amount of yield being converted to principal
    pub reinvested_amount: i128,
    /// New total principal (original + reinvested)
    pub new_total_principal: i128,
}

/// Emitted when funding is paused, either automatically due to a velocity breach
/// or manually by the admin.
#[contractevent]
pub struct FundingPausedEvent {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// The current ledger sequence at which funding was paused.
    pub paused_at_ledger: u32,
    /// The configured max funding rate (0 = no rate configured).
    pub max_funding_rate: u64,
    /// The amount funded in this ledger when the breach occurred.
    pub ledger_funded: i128,
}

/// Emitted when funding is resumed by admin after a pause.
#[contractevent]
pub struct FundingResumedEvent {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// Updated max funding rate (0 = no rate configured).
    pub max_funding_rate: u64,
}

/// Emitted when a yield-bearing token is configured at init.
/// Indexers use this to track yield-wrapped escrows and reconcile
/// base-token → yield-token wrapping events.
#[contractevent]
pub struct YieldTokenBound {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// The yield-bearing token contract (e.g., aUSDC).
    pub yield_token: Address,
    /// The base funding token that gets wrapped.
    pub base_token: Address,
}

/// Emitted during settlement when yield token is unwrapped back to base token.
/// Carries the invoice id, yield token, and the yield earned during the funding period.
#[contractevent]
pub struct YieldUnwrapped {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// The yield-bearing token contract that was unwrapped.
    pub yield_token: Address,
    /// The base funding token received after unwrapping.
    pub base_token: Address,
    /// Raw yield earned (difference between unwrapped and wrapped amounts).
    pub yield_earned: i128,
    /// Ledger timestamp at which the unwrap occurred.
    pub unwrapped_at_ledger_timestamp: u64,
}

/// Emitted when settlement verifies an invoice payment via an external oracle.
/// Carries the oracle contract id and the verification result.
#[contractevent]
pub struct OracleSettlementVerified {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// The oracle contract used for verification.
    pub oracle_contract: Address,
    /// Whether the oracle verified the invoice payment successfully.
    pub verified: bool,
    /// Ledger timestamp at which verification occurred.
    pub verified_at_ledger_timestamp: u64,
}

/// Emitted during settlement when an NFT is minted representing the settled invoice.
/// The NFT can be used as collateral or proof of settlement by the SME.
#[contractevent]
pub struct SettlementNftMinted {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// The NFT contract that minted the token.
    pub nft_contract: Address,
    /// The SME address that receives the NFT.
    pub sme_address: Address,
    /// Settlement date (ledger timestamp).
    pub settlement_date: u64,
    /// Yield paid in basis points at settlement.
    pub yield_paid_bps: i64,
    /// ID of the minted NFT token, queryable by third-party contracts.
    pub nft_token_id: Symbol,
}

/// Wraps an [`ErrorDiagnostic`] for publication, emitted by entrypoints that fail with a
/// diagnosable [`EscrowError`] to give SDKs and integrators a human-readable explanation
/// alongside the raw error code.
#[contractevent]
pub struct ErrorDiagnosticEmitted {
    #[topic]
    pub name: Symbol,
    pub error_code: u32,
    pub message: String,
    pub recovery_action: String,
    pub context: Option<String>,
}

/// Emitted when [`LiquifactEscrow::migrate`] is called, providing version information
/// for operator diagnostics even if the call ultimately fails.
///
/// This event is emitted before any typed error is returned, allowing off-chain tooling
/// to surface version delta information and help operators understand schema compatibility.
/// Schema event version = 1.
#[contractevent]
pub struct MigrationDiagnosticEmitted {
    /// Event schema version for forward compatibility.
    #[topic]
    pub name: Symbol,
    /// Escrow invoice identifier.
    #[topic]
    pub invoice_id: Symbol,
    /// The version stored on-chain (from `DataKey::Version`).
    pub stored_version: u32,
    /// The version provided as a parameter to `migrate` (from_version).
    pub from_version: u32,
    /// The target schema version (SCHEMA_VERSION constant).
    pub target_version: u32,
}

/// Emitted on admin transfer (acceptance) and admin proposal.
///
/// Unifies `AdminProposedEvent` + `AdminTransferredEvent` into one versioned
/// event with ledger timestamp, old and new admin, and the change kind.
/// Schema event version = 1.
#[contractevent]
pub struct AdminChanged {
    /// Event schema version for forward compatibility.
    #[topic]
    pub name: Symbol,
    /// Escrow invoice identifier.
    #[topic]
    pub invoice_id: Symbol,
    /// Address that triggered the change.
    #[topic]
    pub actor: Address,
    /// Ledger timestamp when the change was recorded.
    pub timestamp: u64,
    /// `"proposed"` or `"accepted"`.
    pub kind: Symbol,
    /// Previous admin address.
    pub old_admin: Address,
    /// New admin (or pending) address.
    pub new_admin: Address,
}

/// Emitted when the compliance / legal hold is toggled.
///
/// Carries the actor (admin), ledger timestamp, and active flag.
/// Schema event version = 1.
#[contractevent]
pub struct LegalHoldSet {
    /// Event schema version for forward compatibility.
    #[topic]
    pub name: Symbol,
    /// Escrow invoice identifier.
    #[topic]
    pub invoice_id: Symbol,
    /// Admin that cleared the hold or guardian that confirmed its activation.
    #[topic]
    pub actor: Address,
    /// Ledger timestamp when the hold was toggled.
    pub timestamp: u64,
    /// `1` = hold enabled, `0` = cleared.
    pub active: u32,
}

/// Emitted when the admin pauses (or unpauses) the escrow via
/// [`LiquifactEscrow::set_escrow_paused`].
///
/// While paused, all state‑mutating entrypoints are blocked.
/// Schema event version = 1.
#[contractevent]
pub struct EscrowPaused {
    /// Event schema version for forward compatibility.
    #[topic]
    pub name: Symbol,
    /// Escrow invoice identifier.
    #[topic]
    pub invoice_id: Symbol,
    /// Address that toggled the pause (must be admin).
    #[topic]
    pub actor: Address,
    /// Ledger timestamp when pause was toggled.
    pub timestamp: u64,
    /// `1` = paused, `0` = resumed.
    pub paused: u32,
}

#[contractevent]
pub struct EscrowHealthWarning {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    /// Warning category: 4001–4004 (0 = no warning, for future use).
    pub warning_type: u32,
    /// Current funded principal.
    pub funded_amount: i128,
    /// Funding target.
    pub funding_target: i128,
    /// Funded ratio in basis points: (funded_amount / funding_target) * 10_000
    /// (range 0–10_000+). Clamped to i64::MAX if overflow.
    pub funded_ratio_bps: i64,
    /// Seconds until maturity (may be negative if past maturity).
    /// i64::MAX if no maturity constraint (maturity == 0).
    pub time_to_maturity_secs: i64,
    pub recorded_at_ledger_timestamp: u64,
}

#[contractevent]
pub struct YieldClaimDelegationSet {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub investor: Address,
    #[topic]
    pub delegate: Address,
}

#[contractevent]
pub struct YieldClaimDelegationRevoked {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub investor: Address,
}

#[contractevent]
pub struct ContractUpgraded {
    #[topic]
    pub name: Symbol,
    #[topic]
    pub invoice_id: Symbol,
    pub new_wasm_hash: BytesN<32>,
}

#[contractevent]
pub struct EscrowCloned {
    #[topic]
    pub name: Symbol,
    /// Original (template) escrow's invoice_id.
    #[topic]
    pub template_invoice_id: Symbol,
    /// New escrow's invoice_id.
    #[topic]
    pub new_invoice_id: Symbol,
    /// Admin who performed the clone.
    pub admin: Address,
    /// SME address cloned from template.
    pub sme_address: Address,
    /// Yield basis points cloned from template.
    pub yield_bps: i64,
    /// Maturity timestamp cloned from template.
    pub maturity: u64,
    /// New amount for the cloned escrow.
    pub new_amount: i128,
}
