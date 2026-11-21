use super::*;

/// Stable typed errors emitted by karis-ky escrow entrypoints.
///
/// Codes are append-only: never reuse or renumber a variant. Client SDKs should branch on the
/// numeric code rather than legacy panic strings. See `docs/escrow-error-messages.md`.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EscrowError {
    /// [`LiquifactEscrow::init`] rejected a non-positive invoice amount.
    AmountMustBePositive = 1,
    /// [`LiquifactEscrow::init`] rejected `yield_bps` outside `0..=10_000`.
    YieldBpsOutOfRange = 2,
    /// [`LiquifactEscrow::init`] called when escrow storage already exists.
    EscrowAlreadyInitialized = 3,
    /// [`LiquifactEscrow::init`] rejected an `invoice_id` outside the allowed length range.
    InvoiceIdInvalidLength = 4,
    /// [`LiquifactEscrow::init`] rejected an `invoice_id` with disallowed characters.
    InvoiceIdInvalidCharset = 5,
    /// [`LiquifactEscrow::init`] configured `min_contribution` but it is not positive.
    MinContributionNotPositive = 6,
    /// [`LiquifactEscrow::init`] configured `min_contribution` above the target hint.
    MinContributionExceedsAmount = 7,
    /// [`LiquifactEscrow::init`] configured `max_unique_investors` but it is not positive.
    MaxUniqueInvestorsNotPositive = 8,
    /// [`LiquifactEscrow::init`] configured `max_per_investor` but it is not positive.
    MaxPerInvestorNotPositive = 9,
    /// [`LiquifactEscrow::init`] rejected a tier with `yield_bps` outside `0..=10_000`.
    TierYieldOutOfRange = 10,
    /// [`LiquifactEscrow::init`] rejected a tier yield below the base `yield_bps`.
    TierYieldBelowBase = 11,
    /// [`LiquifactEscrow::init`] rejected tiers whose `min_lock_secs` are not strictly increasing.
    TierLockNotIncreasing = 12,
    /// [`LiquifactEscrow::init`] rejected tiers whose `yield_bps` decrease across tiers.
    TierYieldNotNonDecreasing = 13,

    /// Escrow storage is missing; entrypoint requires prior [`LiquifactEscrow::init`].
    EscrowNotInitialized = 20,
    /// [`DataKey::FundingToken`] is unset (escrow not fully initialized).
    FundingTokenNotSet = 21,
    /// [`DataKey::Treasury`] is unset (escrow not fully initialized).
    TreasuryNotSet = 22,

    /// [`LiquifactEscrow::sweep_terminal_dust`] blocked while a legal hold is active.
    LegalHoldBlocksTreasuryDustSweep = 30,
    /// [`LiquifactEscrow::sweep_terminal_dust`] received a non-positive sweep amount.
    SweepAmountNotPositive = 31,
    /// [`LiquifactEscrow::sweep_terminal_dust`] exceeded [`MAX_DUST_SWEEP_AMOUNT`].
    SweepAmountExceedsMax = 32,
    /// [`LiquifactEscrow::sweep_terminal_dust`] called before a terminal escrow status.
    DustSweepNotTerminal = 33,
    /// [`LiquifactEscrow::sweep_terminal_dust`] found no funding-token balance to sweep.
    NoFundingTokenBalanceToSweep = 34,
    /// [`LiquifactEscrow::sweep_terminal_dust`] computed an effective sweep amount of zero.
    EffectiveSweepAmountZero = 35,
    /// Token transfer wrapper received a non-positive amount (see `external_calls`).
    TransferAmountNotPositive = 36,
    /// Token transfer wrapper found insufficient sender balance before transfer.
    InsufficientTokenBalanceBeforeTransfer = 37,
    /// Token transfer wrapper detected sender balance delta underflow.
    SenderBalanceUnderflow = 38,
    /// Token transfer wrapper detected recipient balance delta underflow.
    RecipientBalanceUnderflow = 39,
    /// Token transfer wrapper detected sender spent amount differs from requested transfer.
    SenderBalanceDeltaMismatch = 40,
    /// Token transfer wrapper detected recipient received amount differs from requested transfer.
    RecipientBalanceDeltaMismatch = 41,
    /// Sweep would reduce the contract balance below outstanding investor liabilities.
    /// `balance - sweep_amt` must be `>= funded_amount - distributed_principal`.
    SweepExceedsLiabilityFloor = 42,

    /// Legacy duplicate-bind code retained for compatibility; no longer emitted.
    PrimaryAttestationAlreadyBound = 50,
    /// [`LiquifactEscrow::append_attestation_digest`] exceeded [`MAX_ATTESTATION_APPEND_ENTRIES`].
    AttestationAppendLogCapacityReached = 51,
    /// [`LiquifactEscrow::bind_primary_attestation_hash`] received a digest that is not exactly 32 bytes.
    InvalidAttestationHashLength = 52,
    /// [`LiquifactEscrow::bind_primary_attestation_hash`] called when a primary hash exists.
    AttestationHashAlreadyBound = 53,

    /// [`LiquifactEscrow::record_sme_collateral_commitment`] received a non-positive amount.
    CollateralAmountNotPositive = 60,
    /// [`LiquifactEscrow::record_sme_collateral_commitment`] received an empty asset symbol.
    CollateralAssetEmpty = 61,
    /// [`LiquifactEscrow::record_sme_collateral_commitment`] received a timestamp before the stored record.
    CollateralTimestampBackwards = 62,
    /// [`LiquifactEscrow::record_sme_collateral_commitment`] called after settlement; updates are not
    /// permitted once the escrow has reached settled (2), withdrawn (3), or cancelled (4) status.
    CollateralUpdateAfterSettlement = 63,

    /// [`LiquifactEscrow::set_investors_allowlisted`] received an empty batch.
    InvestorBatchEmpty = 70,
    /// [`LiquifactEscrow::set_investors_allowlisted`] exceeded [`MAX_INVESTOR_ALLOWLIST_BATCH`].
    InvestorBatchTooLarge = 71,
    /// [`LiquifactEscrow::fund_batch`] received an empty entries vector.
    FundingBatchEmpty = 82,
    /// [`LiquifactEscrow::fund_batch`] exceeded [`MAX_FUND_BATCH`].
    FundingBatchTooLarge = 83,
    /// [`LiquifactEscrow::update_funding_target`] received a non-positive target.
    TargetNotPositive = 72,
    /// [`LiquifactEscrow::update_funding_target`] called while escrow is not open.
    TargetUpdateNotOpen = 73,
    /// [`LiquifactEscrow::update_funding_target`] set target below already-funded principal.
    TargetBelowFundedAmount = 74,
    /// [`LiquifactEscrow::lower_max_unique_investors`] called while escrow is not open.
    CapLowerNotOpen = 75,
    /// [`LiquifactEscrow::lower_max_unique_investors`] called with no investor cap configured.
    NoInvestorCapConfigured = 76,
    /// [`LiquifactEscrow::lower_max_unique_investors`] did not strictly lower the cap.
    NewCapNotLower = 77,
    /// [`LiquifactEscrow::lower_max_unique_investors`] set cap below current unique funder count.
    NewCapBelowCurrentFunderCount = 78,
    /// [`LiquifactEscrow::update_maturity`] called while escrow is not open.
    MaturityUpdateNotOpen = 79,
    /// [`LiquifactEscrow::propose_admin`] nominated the current admin address.
    NewAdminSameAsCurrent = 80,

    /// [`LiquifactEscrow::migrate`] `from_version` does not match stored version.
    MigrationVersionMismatch = 90,
    /// [`LiquifactEscrow::migrate`] called at or above [`SCHEMA_VERSION`].
    AlreadyCurrentSchemaVersion = 91,
    /// [`LiquifactEscrow::migrate`] has no implemented path from the requested version.
    NoMigrationPath = 92,

    /// [`LiquifactEscrow::fund`] / [`LiquifactEscrow::fund_with_commitment`] received non-positive amount.
    FundingAmountNotPositive = 100,
    /// Funding amount is below configured `min_contribution`.
    FundingBelowMinContribution = 101,
    /// Funding blocked while a legal hold is active.
    LegalHoldBlocksFunding = 102,
    /// Funding attempted while escrow is not in open status.
    EscrowNotOpenForFunding = 103,
    /// Allowlist gate active and investor address is not allowlisted.
    InvestorNotAllowlisted = 104,
    /// Adding funding would overflow the investor's stored contribution.
    InvestorContributionOverflow = 105,
    /// Funding would exceed configured `max_per_investor`.
    InvestorContributionExceedsCap = 106,
    /// A new investor would exceed configured `max_unique_investors`.
    UniqueInvestorCapReached = 107,
    /// [`LiquifactEscrow::fund_with_commitment`] called after investor already has principal.
    TieredSecondDeposit = 108,
    /// Computing investor claim-not-before timestamp would overflow.
    InvestorClaimTimeOverflow = 109,
    /// Adding funding would overflow escrow `funded_amount`.
    FundedAmountOverflow = 110,
    /// Commitment lock would push `now + committed_lock_secs` past the escrow maturity.
    /// Reject at deposit time so a settled escrow cannot hold an investor's payout
    /// claim hostage beyond the point where principal is due.
    CommitmentLockExceedsMaturity = 111,

    /// [`LiquifactEscrow::settle`] blocked while a legal hold is active.
    LegalHoldBlocksSettlement = 120,
    /// [`LiquifactEscrow::settle`] called before escrow reached funded status.
    SettlementNotFunded = 121,
    /// [`LiquifactEscrow::settle`] called before configured maturity timestamp.
    MaturityNotReached = 122,
    /// [`LiquifactEscrow::withdraw`] blocked while a legal hold is active.
    LegalHoldBlocksWithdrawal = 123,
    /// [`LiquifactEscrow::withdraw`] called before escrow reached funded status.
    WithdrawalNotFunded = 124,
    /// [`LiquifactEscrow::claim_investor_payout`] blocked while a legal hold is active.
    LegalHoldBlocksInvestorClaims = 125,
    /// [`LiquifactEscrow::claim_investor_payout`] for an address with zero contribution.
    NoContributionToClaim = 126,
    /// [`LiquifactEscrow::claim_investor_payout`] before escrow is settled.
    InvestorClaimNotSettled = 127,
    /// [`LiquifactEscrow::claim_investor_payout`] before tier commitment lock expires.
    InvestorCommitmentLockNotExpired = 128,
    /// Checked arithmetic overflow in [`LiquifactEscrow::compute_investor_payout`].
    ComputePayoutArithmeticOverflow = 129,

    /// [`LiquifactEscrow::cancel_funding`] blocked while a legal hold is active.
    LegalHoldBlocksCancelFunding = 140,
    /// [`LiquifactEscrow::cancel_funding`] called while escrow is not open.
    CancelFundingNotOpen = 141,
    /// [`LiquifactEscrow::refund`] called while escrow is not cancelled.
    RefundNotCancelled = 142,
    /// [`LiquifactEscrow::refund`] for an address with zero contribution.
    NoContributionToRefund = 143,

    /// `clear_legal_hold` was called without a prior `request_legal_hold_clear`.
    LegalHoldClearRequestMissing = 150,
    /// The two-phase legal-hold clear delay has not elapsed yet.
    LegalHoldClearNotReady = 151,
    /// Computing the legal-hold clear ready-at timestamp would overflow.
    LegalHoldClearDelayOverflow = 152,
    /// Funding deadline has passed, new deposits are rejected.
    FundingDeadlinePassed = 153,
    /// [`LiquifactEscrow::set_legal_hold`] called on an escrow in terminal status (settled, withdrawn, cancelled, or archived).
    LegalHoldSetOnTerminalEscrow = 154,

    /// A legal hold blocks rotating the beneficiary (SME) address.
    LegalHoldBlocksBeneficiaryRotation = 160,
    /// Beneficiary rotation was attempted while the escrow was not in a
    /// pre-settlement state (`status` must be 0 = open or 1 = funded).
    RotationNotOpen = 161,
    /// [`LiquifactEscrow::claim_investor_payout`] detected yield slippage exceeding configured threshold.
    YieldSlippageDetected = 162,
    /// Yield slippage threshold is out of valid range (0..=10_000 bps).
    YieldSlippageThresholdOutOfRange = 163,
    /// The proposed new SME address is identical to the current beneficiary.
    NewSmeSameAsCurrent = 181,

    /// Attempted to accept admin role when no pending admin exists.
    NoPendingAdmin = 182,
    /// The contract's funding-token balance is less than `funded_amount` at withdraw time.
    /// Funds must be custodied in this contract before the SME can pull them.
    InsufficientContractBalance = 183,

    /// [`LiquifactEscrow::fund`] / [`LiquifactEscrow::fund_with_commitment`] blocked while a dispute pause is active.
    DisputePausedBlocksFunding = 165,
    /// [`LiquifactEscrow::settle`] blocked while a dispute pause is active.
    DisputePausedBlocksSettlement = 166,
    /// [`LiquifactEscrow::withdraw`] blocked while a dispute pause is active.
    DisputePausedBlocksWithdrawal = 167,
    /// [`LiquifactEscrow::claim_investor_payout`] blocked while a dispute pause is active.
    DisputePausedBlocksInvestorClaims = 168,
    /// [`LiquifactEscrow::pause_dispute`] received a non-positive pause duration in seconds.
    DisputePauseDurationNotPositive = 169,
    /// [`LiquifactEscrow::pause_dispute`] received a pause duration exceeding the maximum allowed window.
    /// See [`MAX_DISPUTE_PAUSE_DURATION_SECS`] for the upper bound.
    DisputePauseDurationExceedsMax = 206,
    /// [`LiquifactEscrow::pause_dispute`] received an empty dispute ticket reference.
    DisputeTicketIdEmpty = 170,
    /// [`LiquifactEscrow::resume_dispute`] called when no dispute pause is active.
    NoPauseActive = 171,
    /// Computed ledger timestamp would overflow (e.g., `now + duration > u64::MAX`).
    LedgerTimestampOverflow = 172,

    /// [`LiquifactEscrow::fund`] / [`LiquifactEscrow::fund_with_commitment`] rejected an investor
    /// that is not verified by the configured [`DataKey::KycProviderContract`].
    InvestorNotVerified = 173,
    /// A sanctions-screening check rejected an address per the configured
    /// [`DataKey::SanctionsProvider`].
    SanctionsScreeningFailed = 205,

    /// Caller's effective [`AdminRole`] (from [`DataKey::AdminRoles`], or the implicit `Full`
    /// fallback for [`InvoiceEscrow::admin`]) does not meet the entrypoint's minimum required role.
    InsufficientAdminRole = 174,
    /// [`LiquifactEscrow::set_admin_roles`] received an empty or oversized role list, or a
    /// duplicate address within it.
    InvalidAdminRoleList = 175,

    /// [`LiquifactEscrow::init_multisig_policy`] received a `threshold` of zero or greater than
    /// the number of configured signers.
    MultisigThresholdInvalid = 176,
    /// [`LiquifactEscrow::init_multisig_policy`] received an empty, oversized, or
    /// duplicate-containing `signers` list.
    MultisigSignerListInvalid = 177,
    /// [`LiquifactEscrow::init_multisig_policy`] received an empty `operations` list.
    MultisigOperationsListEmpty = 178,
    /// A multisig-gated entrypoint received fewer distinct policy-member signatures than the
    /// configured [`MultisigPolicy::threshold`].
    MultisigInsufficientSigners = 179,
    /// A multisig-gated entrypoint received a signer address that is not a member of the
    /// configured [`MultisigPolicy::signers`], or the same signer listed more than once.
    MultisigSignerNotAuthorized = 180,
    /// The source escrow must be in a settled terminal state before the investor can roll yield.
    ReinvestSourceEscrowNotSettled = 181,
    /// The target escrow must still be in a funding/open flow before accepting reinvested yield.
    ReinvestTargetEscrowNotFunding = 182,
    /// The reinvestment amount must be strictly positive.
    ReinvestAmountNotPositive = 183,
    /// The investor does not hold enough accrued yield in the source escrow to complete the reinvestment.
    ReinvestYieldInsufficient = 184,
    /// [`LiquifactEscrow::withdraw`] called by a caller that is not the registered SME address.
    UnauthorizedWithdrawer = 185,
    /// A guardian-configured escrow requires admin proposal and guardian confirmation to activate a hold.
    LegalHoldRequiresGuardianConfirmation = 207,
    /// No legal-hold proposal is pending for guardian confirmation.
    LegalHoldProposalMissing = 208,
    /// A pending legal-hold proposal has expired.
    LegalHoldProposalExpired = 209,
    /// The supplied guardian or admin does not match the configured escrow authority.
    LegalHoldAuthorityMismatch = 210,
    /// No guardian is configured for this escrow.
    LegalHoldGuardianNotConfigured = 211,
    /// A legal hold is already active.
    LegalHoldAlreadyActive = 212,
    /// The configured legal-hold guardian must be distinct from the escrow admin.
    LegalHoldGuardianSameAsAdmin = 213,
}

#[inline(always)]
pub(crate) fn fail(env: &Env, error: EscrowError) -> ! {
    panic_with_error!(env, error)
}

#[inline(always)]
pub(crate) fn ensure(env: &Env, condition: bool, error: EscrowError) {
    if !condition {
        fail(env, error);
    }
}
