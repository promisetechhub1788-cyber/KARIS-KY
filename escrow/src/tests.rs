#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    clippy::needless_borrow,
    clippy::len_zero,
    clippy::explicit_counter_loop
)]
#[allow(unused_imports)]
use super::{
    AttestationBoundEvt, AttestationDigestRevoked, CollateralRecordedEvt, ContractLimits, DataKey,
    EscrowError, EscrowFunded, EscrowInitialized, FundingTargetUpdated, LiquifactEscrow,
    LiquifactEscrowClient, MaxUniqueInvestorsCapLowered, YieldClaimDelegationRevoked,
    YieldClaimDelegationSet, YieldTier, LEGAL_HOLD_PROPOSAL_TTL_SECS,
    MAX_ATTESTATION_APPEND_ENTRIES, MAX_DUST_SWEEP_AMOUNT, MAX_FUND_BATCH, SCHEMA_VERSION,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger as _},
    token::{StellarAssetClient, TokenClient},
    Address, Env, Error, Event, InvokeError, String, Val, Vec as SorobanVec,
};
use std::fmt::Debug;

pub(crate) fn assert_contract_error<T, E>(
    result: Result<Result<T, E>, Result<Error, InvokeError>>,
    expected: EscrowError,
) where
    T: Debug,
    E: Debug,
{
    let expected_code = expected as u32;
    match result {
        Err(Ok(error)) => {
            assert_eq!(error, Error::from_contract_error(expected_code));
        }
        Err(Err(InvokeError::Contract(code))) => {
            assert_eq!(code, expected_code);
        }
        other => panic!("expected ContractError({expected_code}), got {other:?}"),
    }
}

// Focused test tree for escrow behavior. Shared helpers live here so feature
// modules stay assertion-focused and each test still owns a fresh Env.
mod admin;
mod attestations;
mod batch_claim;
mod cap_validation;
mod clone;
mod coverage;
mod dos_analysis;
mod external_calls;
mod external_calls_mocked;
mod funding;
mod get_limits;
mod health_and_events;
mod history_pagination;
mod init;
mod integration;
mod legal_hold;
mod properties;
mod properties_funding;
mod secure_rng;
mod settlement;
mod tier_base_yield;
mod tokenomics;
mod upgrade_compat;
mod validation;
mod yield_distribution;
mod yield_tier_table_read;

/// Registers a new escrow contract instance and returns its contract id.
pub fn deploy_id(env: &Env) -> Address {
    env.register(LiquifactEscrow, ())
}

pub fn deploy(env: &Env) -> LiquifactEscrowClient<'_> {
    let id = deploy_id(env);
    LiquifactEscrowClient::new(env, &id)
}

#[allow(dead_code)]
pub fn deploy_with_id(env: &Env) -> (Address, LiquifactEscrowClient<'_>) {
    let id = deploy_id(env);
    let client = LiquifactEscrowClient::new(env, &id);
    (id, client)
}

pub fn setup(env: &Env) -> (LiquifactEscrowClient<'_>, Address, Address) {
    let mut ledger_info = env.ledger().get();
    ledger_info.timestamp = 12345;
    ledger_info.sequence_number = 100;
    env.ledger().set(ledger_info);
    env.mock_all_auths();
    let client = deploy(env);
    let admin = Address::generate(env);
    let sme = Address::generate(env);
    (client, admin, sme)
}

pub fn free_addresses(env: &Env) -> (Address, Address) {
    (Address::generate(env), Address::generate(env))
}

pub struct StellarTestToken<'a> {
    /// Contract id for the standard Stellar asset token.
    pub id: Address,
    /// SEP-41 interface (the same interface the escrow uses in `external_calls`).
    pub token: TokenClient<'a>,
    /// Test-only admin client used for minting balances into accounts/contracts.
    pub stellar: StellarAssetClient<'a>,
}

/// Install a **standard** Stellar asset token contract (Soroban StellarAsset contract v2).
///
/// This is intentionally used for tests that require "well-behaved" SEP-41 semantics:
/// - No fee-on-transfer / rebasing / callback side-effects.
/// - `balance` deltas match transfer amounts (as asserted by `external_calls` wrappers).
///
/// **Out of scope:** non-standard/malicious token economics; see `escrow/src/external_calls.rs`
/// and `docs/ESCROW_TOKEN_INTEGRATION_CHECKLIST.md`.
pub fn install_stellar_asset_token<'a>(env: &'a Env) -> StellarTestToken<'a> {
    let sac = env.register_stellar_asset_contract_v2(Address::generate(env));
    let id = sac.address();
    StellarTestToken {
        id: id.clone(),
        token: TokenClient::new(env, &id),
        stellar: StellarAssetClient::new(env, &id),
    }
}

#[allow(dead_code)]
pub fn default_init(client: &LiquifactEscrowClient<'_>, env: &Env, admin: &Address, sme: &Address) {
    let (token, treasury) = free_addresses(env);
    client.init(
        admin,
        &soroban_sdk::String::from_str(env, "INV001"),
        sme,
        &100_000_000_000i128,
        &800i64,
        &0u64,
        &token,
        &None,
        &treasury,

/// Test: Verify that all EscrowError variants have unique discriminant codes.
///
/// This test iterates through all known EscrowError variants and asserts that
/// no two variants share the same numeric discriminant. This is a compile-time
/// invariant that is checked at runtime for safety.
///
/// **Rationale:** Rust's `#[contracterror]` macro does not prevent duplicate
/// discriminants in the enum definition (E0081 is raised by rustc, not by the macro).
/// This test makes the uniqueness requirement explicit and catches any future
/// regressions if variants are accidentally re-assigned to the same code.
#[test]
fn test_escrow_error_codes_are_unique() {
    // Create a set of all error codes by converting each variant to its numeric value.
    // This list must be kept in sync with all EscrowError variants; if you add a new
    // variant, add its code here as well.
    let error_codes = vec![
        // Init-time errors (1-13)
        EscrowError::AmountMustBePositive as u32,
        EscrowError::YieldBpsOutOfRange as u32,
        EscrowError::EscrowAlreadyInitialized as u32,
        EscrowError::InvoiceIdInvalidLength as u32,
        EscrowError::InvoiceIdInvalidCharset as u32,
        EscrowError::MinContributionNotPositive as u32,
        EscrowError::MinContributionExceedsAmount as u32,
        EscrowError::MaxUniqueInvestorsNotPositive as u32,
        EscrowError::MaxPerInvestorNotPositive as u32,
        EscrowError::TierYieldOutOfRange as u32,
        EscrowError::TierYieldBelowBase as u32,
        EscrowError::TierLockNotIncreasing as u32,
        EscrowError::TierYieldNotNonDecreasing as u32,
        // Init state checks (20-22)
        EscrowError::EscrowNotInitialized as u32,
        EscrowError::FundingTokenNotSet as u32,
        EscrowError::TreasuryNotSet as u32,
        // Sweep/dust errors (30-42)
        EscrowError::LegalHoldBlocksTreasuryDustSweep as u32,
        EscrowError::SweepAmountNotPositive as u32,
        EscrowError::SweepAmountExceedsMax as u32,
        EscrowError::DustSweepNotTerminal as u32,
        EscrowError::NoFundingTokenBalanceToSweep as u32,
        EscrowError::EffectiveSweepAmountZero as u32,
        EscrowError::TransferAmountNotPositive as u32,
        EscrowError::InsufficientTokenBalanceBeforeTransfer as u32,
        EscrowError::SenderBalanceUnderflow as u32,
        EscrowError::RecipientBalanceUnderflow as u32,
        EscrowError::SenderBalanceDeltaMismatch as u32,
        EscrowError::RecipientBalanceDeltaMismatch as u32,
        EscrowError::SweepExceedsLiabilityFloor as u32,
        // Attestation errors (50-52)
        EscrowError::PrimaryAttestationAlreadyBound as u32,
        EscrowError::AttestationAppendLogCapacityReached as u32,
        EscrowError::InvalidAttestationHashLength as u32,
        // Collateral errors (60-63)
        EscrowError::CollateralAmountNotPositive as u32,
        EscrowError::CollateralAssetEmpty as u32,
        EscrowError::CollateralTimestampBackwards as u32,
        EscrowError::CollateralUpdateAfterSettlement as u32,
        // Allowlist/batch/cap errors (70-83)
        EscrowError::InvestorBatchEmpty as u32,
        EscrowError::InvestorBatchTooLarge as u32,
        EscrowError::FundingBatchEmpty as u32,
        EscrowError::FundingBatchTooLarge as u32,
        EscrowError::TargetNotPositive as u32,
        EscrowError::TargetUpdateNotOpen as u32,
        EscrowError::TargetBelowFundedAmount as u32,
        EscrowError::CapLowerNotOpen as u32,
        EscrowError::NoInvestorCapConfigured as u32,
        EscrowError::NewCapNotLower as u32,
        EscrowError::NewCapBelowCurrentFunderCount as u32,
        EscrowError::MaturityUpdateNotOpen as u32,
        EscrowError::NewAdminSameAsCurrent as u32,
        // Migration errors (90-92)
        EscrowError::MigrationVersionMismatch as u32,
        EscrowError::AlreadyCurrentSchemaVersion as u32,
        EscrowError::NoMigrationPath as u32,
        // Funding errors (100-111)
        EscrowError::FundingAmountNotPositive as u32,
        EscrowError::FundingBelowMinContribution as u32,
        EscrowError::LegalHoldBlocksFunding as u32,
        EscrowError::EscrowNotOpenForFunding as u32,
        EscrowError::InvestorNotAllowlisted as u32,
        EscrowError::InvestorContributionOverflow as u32,
        EscrowError::InvestorContributionExceedsCap as u32,
        EscrowError::UniqueInvestorCapReached as u32,
        EscrowError::TieredSecondDeposit as u32,
        EscrowError::InvestorClaimTimeOverflow as u32,
        EscrowError::FundedAmountOverflow as u32,
        EscrowError::CommitmentLockExceedsMaturity as u32,
        // Settlement/withdrawal/claim errors (120-129)
        EscrowError::LegalHoldBlocksSettlement as u32,
        EscrowError::SettlementNotFunded as u32,
        EscrowError::MaturityNotReached as u32,
        EscrowError::LegalHoldBlocksWithdrawal as u32,
        EscrowError::WithdrawalNotFunded as u32,
        EscrowError::LegalHoldBlocksInvestorClaims as u32,
        EscrowError::NoContributionToClaim as u32,
        EscrowError::InvestorClaimNotSettled as u32,
        EscrowError::InvestorCommitmentLockNotExpired as u32,
        EscrowError::ComputePayoutArithmeticOverflow as u32,
        // Cancellation/refund errors (140-143)
        EscrowError::LegalHoldBlocksCancelFunding as u32,
        EscrowError::CancelFundingNotOpen as u32,
        EscrowError::RefundNotCancelled as u32,
        EscrowError::NoContributionToRefund as u32,
        // Legal hold clearance errors (150-152)
        EscrowError::LegalHoldClearRequestMissing as u32,
        EscrowError::LegalHoldClearNotReady as u32,
        EscrowError::LegalHoldClearDelayOverflow as u32,
        // Beneficiary rotation and yield slippage errors (160-163)
        EscrowError::LegalHoldBlocksBeneficiaryRotation as u32,
        EscrowError::RotationNotOpen as u32,
        EscrowError::YieldSlippageDetected as u32,
        EscrowError::YieldSlippageThresholdOutOfRange as u32,
        // Funding deadline and admin errors (164, 181-182)
        EscrowError::FundingDeadlinePassed as u32,
        EscrowError::NewSmeSameAsCurrent as u32,
        EscrowError::NoPendingAdmin as u32,
        EscrowError::InsufficientContractBalance as u32,
        // Dispute pause errors (165-172)
        EscrowError::DisputePausedBlocksFunding as u32,
        EscrowError::DisputePausedBlocksSettlement as u32,
        EscrowError::DisputePausedBlocksWithdrawal as u32,
        EscrowError::DisputePausedBlocksInvestorClaims as u32,
        EscrowError::DisputePauseDurationNotPositive as u32,
        EscrowError::DisputeTicketIdEmpty as u32,
        EscrowError::NoPauseActive as u32,
        EscrowError::LedgerTimestampOverflow as u32,
        // KYC and role errors (173-175)
        EscrowError::InvestorNotVerified as u32,
        EscrowError::InsufficientAdminRole as u32,
        EscrowError::InvalidAdminRoleList as u32,
        // Multisig policy errors (176-180)
        EscrowError::MultisigThresholdInvalid as u32,
        EscrowError::MultisigSignerListInvalid as u32,
        EscrowError::MultisigOperationsListEmpty as u32,
        EscrowError::MultisigInsufficientSigners as u32,
        EscrowError::MultisigSignerNotAuthorized as u32,
    ];

    // Check for duplicates by using a set-like structure.
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();

    for code in &error_codes {
        if !seen.insert(code) {
            duplicates.push(code);
        }
    }

    // Assert no duplicates exist.
    assert!(
        duplicates.is_empty(),
        "Found duplicate EscrowError discriminants: {:?}",
        duplicates
    );
}
        &None,
        &None,
        &None,
        &None,
        &None,
        &None, // No max funding rate
        &None, // No yield slippage threshold
        &None, // No settlement notifier
        &None, // No KYC provider
        &None, // No admin roles
    );
}

#[allow(dead_code)]
pub const TARGET: i128 = 100_000_000_000i128;

/// Create a **new** escrow contract backed by a real Stellar asset contract (SAC),
/// initialise it with a funded target, fund it to exactly `target`, and mint `target`
/// tokens into the escrow contract address so that `withdraw()` can actually transfer
/// them.
///
/// Returns `(client, escrow_id, sme, token_client)`.  The caller must have called
/// `env.mock_all_auths()` (or equivalent) before invoking this helper.
#[allow(dead_code)]
pub fn init_and_fund_with_real_token<'a>(
    env: &'a Env,
    target: i128,
    invoice_id: &str,
) -> (LiquifactEscrowClient<'a>, Address, Address) {
    let sac = env.register_stellar_asset_contract_v2(Address::generate(env));
    let token_id = sac.address();
    let sac_admin = StellarAssetClient::new(env, &token_id);

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(env, &escrow_id);
    let admin = Address::generate(env);
    let sme = Address::generate(env);
    let treasury = Address::generate(env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(env, invoice_id),
        &sme,
        &target,
        &800i64,
        &0u64,
        &token_id,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None, // No yield slippage threshold
        &None, // No settlement notifier
    );

    let investor = Address::generate(env);
    client.fund(&investor, &target);

    // Mint funded_amount into the escrow so withdraw() can actually transfer tokens.
    sac_admin.mint(&escrow_id, &target);

    (client, escrow_id, sme)
}
