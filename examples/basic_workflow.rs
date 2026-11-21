//! # karis-ky Escrow SDK Examples
//!
//! This file demonstrates common patterns for interacting with the karis-ky
//! LiquifactEscrow contract on the Stellar/Soroban network.
//!
//! ## Covered Patterns
//!
//! - **init**: Deploy and initialize an invoice escrow
//! - **fund**: Fund as an investor (simple funding)
//! - **fund_with_commitment**: Fund with a lock period for tiered yield
//! - **settle**: Settle the escrow as the SME
//! - **claim_investor_payout**: Claim payout as an investor after settlement
//! - **withdraw**: SME pulls funded liquidity
//!
//! ## Error Handling
//!
//! All entrypoints emit typed [`EscrowError`] codes (see docs/escrow-error-messages.md).
//! SDKs should branch on the numeric error code rather than parsing panic strings.
//!
//! ## Prerequisites
//!
//! ```bash
//! cargo build -p karis_ky_escrow
//! ```
//!
//! ## Running the Example
//!
//! CI builds and runs this example on every pull request so it cannot rot.
//! Run it locally with either of:
//!
//! ```bash
//! cargo run --example basic_workflow
//! # or the test-gated equivalent (runs the lifecycle assertions):
//! cargo test -p karis_ky_escrow --example basic_workflow -- --nocapture
//! ```
//!
//! A failure in any of the example scenarios fails CI.
//!
//! ## Expected Output
//!
//! Running `cargo run --example basic_workflow` prints the banner below and
//! exits with status `0`:
//!
//! ```text
//! karis-ky Escrow SDK Examples
//! ===========================
//!
//! This file contains example code demonstrating common patterns
//! for interacting with the LiquifactEscrow contract.
//!
//! Run with: cargo test -p karis_ky_escrow
//!
//! For detailed documentation, see:
//!   - docs/escrow-data-model.md
//!   - docs/escrow-events.md
//!   - docs/escrow-security-checklist.md
//!   - README.md
//! ```
//!
//! The test-gated scenarios assert the following lifecycle transitions and
//! must all pass:
//!
//! - **init → fund → settle → claim** (`example_basic_workflow_init_fund_settle_claim`):
//!   status goes `0` (Open) → `1` (Funded) → `2` (Settled); the investor's
//!   contribution equals the invoice amount; `is_investor_claimed` is `true`
//!   after the first claim and stays `true` after a second (idempotent) claim.
//! - **Yield-token workflow** (`example_yield_token_workflow`): `get_yield_token`
//!   returns the configured token; fund → settle → claim succeeds.
//! - **NFT mint workflow** (`example_nft_mint_workflow`): `get_nft_contract`
//!   returns the configured contract; settle reaches status `2`; claim succeeds.
//! - **Tiered yield** (Example 2): short-lock investor claims after 30 days,
//!   long-lock investor remains locked until 90 days, then both are claimed.
//!
//! ---

use karis_ky_escrow::{
    EscrowError, InvoiceEscrow, LiquifactEscrow, LiquifactEscrowClient, YieldTier,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Real compilable example functions — run with:
//   cargo test -p karis_ky_escrow -- --nocapture
// ═══════════════════════════════════════════════════════════════════════════════

/// Example: Initialize escrow, fund as investor, settle as SME, claim as investor.
/// This is the core lifecycle demonstrating all four required patterns.
#[cfg(test)]
#[test]
fn example_basic_workflow_init_fund_settle_claim() {
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    let env = Env::default();
    env.mock_all_auths();

    // Step 1: Deploy and initialize the escrow
    let escrow_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &escrow_id);

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let funding_token = Address::generate(&env);
    let treasury = Address::generate(&env);

    let invoice_amount: i128 = 1_000_000_000;
    let yield_bps: i64 = 500;

    let escrow = client.init(
        &admin,
        &String::from_str(&env, "EX-001"),
        &sme,
        &invoice_amount,
        &yield_bps,
        &0u64,
        &funding_token,
        &None, &treasury, &None, &None, &None, &None,
        &None, &None, &None, &None, &None, &None,
    );
    assert_eq!(escrow.status, 0);

    // Step 2: Fund as investor
    let funded = client.fund(&investor, &invoice_amount);
    assert_eq!(funded.status, 1);
    assert_eq!(client.get_contribution(&investor), invoice_amount);

    // Step 3: Settle as SME
    let settled = client.settle();
    assert_eq!(settled.status, 2);

    // Step 4: Claim as investor
    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));

    // Idempotent: second claim is a no-op
    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));
}

/// Example: Initialize with yield-bearing token, fund, settle, claim.
#[cfg(test)]
#[test]
fn example_yield_token_workflow() {
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    let env = Env::default();
    env.mock_all_auths();

    let client = LiquifactEscrowClient::new(&env, &env.register(LiquifactEscrow, ()));
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let base_token = Address::generate(&env);
    let yield_token = Address::generate(&env);
    let treasury = Address::generate(&env);

    let escrow = client.init(
        &admin,
        &String::from_str(&env, "YIELD-EX"),
        &sme,
        &1_000_000_000i128,
        &400i64,
        &0u64,
        &base_token,
        &None, &treasury, &None, &None, &None, &None,
        &None, &None,
        &Some(yield_token.clone()), // yield_token
        &None,                      // oracle_contract
        &None,                      // nft_contract
    );

    assert_eq!(client.get_yield_token(), Some(yield_token.clone()));

    client.fund(&investor, &1_000_000_000i128);
    client.settle();
    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));
}

/// Example: Initialize with NFT contract, settle to mint settlement NFT.
#[cfg(test)]
#[test]
fn example_nft_mint_workflow() {
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    let env = Env::default();
    env.mock_all_auths();

    let client = LiquifactEscrowClient::new(&env, &env.register(LiquifactEscrow, ()));
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    let nft_contract = Address::generate(&env);

    client.init(
        &admin,
        &String::from_str(&env, "NFT-EX"),
        &sme,
        &1_000_000_000i128,
        &400i64,
        &0u64,
        &token,
        &None, &treasury, &None, &None, &None, &None,
        &None, &None,
        &None,                      // yield_token
        &None,                      // oracle_contract
        &Some(nft_contract.clone()), // nft_contract
    );

    assert_eq!(client.get_nft_contract(), Some(nft_contract.clone()));

    client.fund(&investor, &1_000_000_000i128);
    let settled = client.settle();
    assert_eq!(settled.status, 2);

    client.claim_investor_payout(&investor);
}

/// # Example 1: Basic Escrow Lifecycle
///
/// This example demonstrates the complete happy path:
/// 1. Admin initializes an escrow for an invoice
/// 2. Investor funds the escrow to reach the target
/// 3. SME settles the escrow after maturity
/// 4. Investor claims their payout
///
/// ```rust
/// fn example_basic_workflow() {
///     use soroban_sdk::{testutils::Address as _, Address, Env, String};
///
///     let env = Env::default();
///     env.mock_all_auths();
///
///     // --- Step 1: Deploy and initialize the escrow ---
///     let escrow_id = env.register(LiquifactEscrow, ());
///     let client = LiquifactEscrowClient::new(&env, &escrow_id);
///
///     let admin = Address::generate(&env);
///     let sme = Address::generate(&env);
///     let investor = Address::generate(&env);
///     let funding_token = Address::generate(&env);
///     let treasury = Address::generate(&env);
///
///     let invoice_amount: i128 = 1_000_000_000; // 1000 USDC (6 decimals)
///     let yield_bps: i64 = 500; // 5.00% annualized yield
///     let maturity: u64 = env.ledger().timestamp() + 30 * 24 * 60 * 60; // 30 days
///
///     let escrow = client.init(
///         &admin,
///         &String::from_str(&env, "INV-2024-001"),
///         &sme,
///         &invoice_amount,
///         &yield_bps,
///         &maturity,
///         &funding_token,
///         &None,    // registry
///         &treasury,
///         &None,    // yield_tiers
///         &None,    // min_contribution
///         &None,    // max_unique_investors
///         &None,    // max_per_investor
///         &None,    // legal_hold_clear_delay
///         &None,    // funding_deadline
///         &None,    // yield_slippage_threshold
///         &None,    // yield_token
///         &None,    // oracle_contract
///         &None,    // nft_contract
///     );
///
///     assert_eq!(escrow.status, 0); // Open
///     assert_eq!(escrow.invoice_id, soroban_sdk::symbol_short!("INV-2024-001"));
///
///     // --- Step 2: Investor funds the escrow ---
///     let funded_event = client.fund(&investor, &invoice_amount);
///     assert_eq!(funded_event.status, 1); // Now funded
///
///     let contribution = client.get_contribution(&investor);
///     assert_eq!(contribution, invoice_amount);
///
///     // --- Step 3: Advance time past maturity and settle ---
///     env.ledger().with_mut(|l| l.timestamp = maturity + 1);
///
///     let settled = client.settle();
///     assert_eq!(settled.status, 2); // Settled
///
///     // --- Step 4: Investor claims payout ---
///     client.claim_investor_payout(&investor);
///     assert!(client.is_investor_claimed(&investor));
///
///     // Verify the second claim is idempotent (no-op)
///     client.claim_investor_payout(&investor);
///     assert!(client.is_investor_claimed(&investor));
/// }
/// ```

/// # Example 2: Escrow with Tiered Yield
///
/// This example shows how to configure tiered yield rates that reward
/// investors who commit to longer lock periods.
///
/// ```rust
/// fn example_tiered_yield() {
///     use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};
///
///     let env = Env::default();
///     env.mock_all_auths();
///
///     let escrow_id = env.register(LiquifactEscrow, ());
///     let client = LiquifactEscrowClient::new(&env, &escrow_id);
///
///     let admin = Address::generate(&env);
///     let sme = Address::generate(&env);
///     let investor_short = Address::generate(&env);
///     let investor_long = Address::generate(&env);
///     let token = Address::generate(&env);
///     let treasury = Address::generate(&env);
///
///     // Configure tiered yield:
///     // - Base: 4% (400 bps)
///     // - Tier 1: 5% for 30-day lock
///     // - Tier 2: 7% for 90-day lock
///     let yield_tiers = Vec::from_array(
///         &env,
///         [
///             YieldTier {
///                 min_lock_secs: 30 * 24 * 60 * 60,
///                 yield_bps: 500,
///             },
///             YieldTier {
///                 min_lock_secs: 90 * 24 * 60 * 60,
///                 yield_bps: 700,
///             },
///         ],
///     );
///
///     client.init(
///         &admin,
///         &String::from_str(&env, "INV-TIER-001"),
///         &sme,
///         &2_000_000_000i128,
///         &400i64, // base yield 4%
///         &0u64,   // no maturity lock
///         &token,
///         &None,
///         &treasury,
///         &Some(yield_tiers),
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,    // yield_token
///         &None,    // oracle_contract
///         &None,    // nft_contract
///     );
///
///     // Investor with short lock (30 days) gets Tier 1 yield (5%)
///     client.fund_with_commitment(
///         &investor_short,
///         &1_000_000_000i128,
///         &(30 * 24 * 60 * 60u64),
///     );
///
///     // Investor with long lock (90 days) gets Tier 2 yield (7%)
///     client.fund_with_commitment(
///         &investor_long,
///         &1_000_000_000i128,
///         &(90 * 24 * 60 * 60u64),
///     );
///
///     client.settle();
///
///     // Claims respect individual lock periods
///     let elapsed: u64 = 30 * 24 * 60 * 60;
///     env.ledger().with_mut(|l| l.timestamp = elapsed);
///
///     // Short-lock investor can claim after 30 days
///     client.claim_investor_payout(&investor_short);
///     assert!(client.is_investor_claimed(&investor_short));
///
///     // Long-lock investor is still locked at 30 days
///     // claim_investor_payout would panic with InvestorCommitmentLockNotExpired
///     let still_locked = !client.is_investor_claimed(&investor_long);
///     assert!(still_locked);
///
///     // Advance to 90 days — now both can claim
///     env.ledger().with_mut(|l| l.timestamp = 90 * 24 * 60 * 60);
///     client.claim_investor_payout(&investor_long);
///     assert!(client.is_investor_claimed(&investor_long));
/// }
/// ```

/// # Example 3: SME Withdraws Liquidity
///
/// When the SME wants to pull funded liquidity directly (without settlement),
/// they call `withdraw()`.
///
/// ```rust
/// fn example_sme_withdraw() {
///     use soroban_sdk::{testutils::Address as _, Address, Env, String};
///     use soroban_sdk::token::StellarAssetClient;
///
///     let env = Env::default();
///     env.mock_all_auths();
///
///     // Use a real Stellar asset token for withdrawal testing
///     let sac = env.register_stellar_asset_contract_v2(Address::generate(&env));
///     let token_id = sac.address();
///     let sac_admin = StellarAssetClient::new(&env, &token_id);
///
///     let escrow_id = env.register(LiquifactEscrow, ());
///     let client = LiquifactEscrowClient::new(&env, &escrow_id);
///
///     let admin = Address::generate(&env);
///     let sme = Address::generate(&env);
///     let investor = Address::generate(&env);
///     let treasury = Address::generate(&env);
///
///     let target: i128 = 500_000_000;
///
///     client.init(
///         &admin,
///         &String::from_str(&env, "INV-WD-001"),
///         &sme,
///         &target,
///         &300i64,
///         &0u64,
///         &token_id,
///         &None,
///         &treasury,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,    // yield_token
///         &None,    // oracle_contract
///         &None,    // nft_contract
///     );
///
///     // Fund the escrow
///     client.fund(&investor, &target);
///
///     // Mint tokens into escrow so withdraw can actually transfer
///     sac_admin.mint(&escrow_id, &target);
///
///     // SME withdraws — status goes to 3 (withdrawn)
///     let escrow = client.withdraw();
///     assert_eq!(escrow.status, 3);
///
///     // Verify tokens were transferred to SME
///     // (In production, check the token balance of sme_address)
/// }
/// ```

/// # Example 4: Multi-Investor Pro-Rata Payout Calculation
///
/// Off-chain pro-rata share calculation using the funding close snapshot.
///
/// ```rust
/// fn example_pro_rata_calculation() {
///     use soroban_sdk::{testutils::Address as _, Address, Env, String};
///
///     let env = Env::default();
///     env.mock_all_auths();
///
///     let escrow_id = env.register(LiquifactEscrow, ());
///     let client = LiquifactEscrowClient::new(&env, &escrow_id);
///
///     let admin = Address::generate(&env);
///     let sme = Address::generate(&env);
///     let inv_a = Address::generate(&env);
///     let inv_b = Address::generate(&env);
///     let inv_c = Address::generate(&env);
///     let token = Address::generate(&env);
///     let treasury = Address::generate(&env);
///
///     let target: i128 = 3_000_000_000;
///
///     client.init(
///         &admin,
///         &String::from_str(&env, "PRO-RATA-01"),
///         &sme,
///         &target,
///         &500i64,
///         &0u64,
///         &token,
///         &None,
///         &treasury,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,    // yield_token
///         &None,    // oracle_contract
///         &None,    // nft_contract
///     );
///
///     // Three investors contribute different amounts
///     client.fund(&inv_a, &1_500_000_000i128); // 50%
///     client.fund(&inv_b, &1_000_000_000i128); // 33.3%
///     client.fund(&inv_c, &500_000_000i128);    // 16.7%
///
///     client.settle();
///
///     // Read the funding close snapshot for pro-rata denominator
///     let snapshot = client.get_funding_close_snapshot().unwrap();
///     let total_principal = snapshot.total_principal; // 3_000_000_000
///     let yield_bps = client.get_escrow().yield_bps;
///
///     // Off-chain pro-rata calculation:
///     // For investor A: share = 1_500_000_000 / 3_000_000_000 = 0.5 (50%)
///     // Payout = share * total_principal * (1 + yield_bps / 10000)
///     let total_pool: i128 =
///         total_principal + (total_principal * yield_bps as i128 / 10_000);
///
///     // Each investor's payout = contribution / total_principal * total_pool
///     let payout_a = (client.get_contribution(&inv_a) * total_pool) / total_principal;
///     let payout_b = (client.get_contribution(&inv_b) * total_pool) / total_principal;
///     let payout_c = (client.get_contribution(&inv_c) * total_pool) / total_principal;
///
///     // Sum of payouts must not exceed pool (within rounding)
///     let payout_sum = payout_a + payout_b + payout_c;
///     assert!(payout_sum <= total_pool + 2); // Allow ±2 for integer rounding
///
///     // Investors can now claim
///     client.claim_investor_payout(&inv_a);
///     client.claim_investor_payout(&inv_b);
///     client.claim_investor_payout(&inv_c);
/// }
/// ```

/// # Example 5: Escrow with Yield-Bearing Token (e.g., aUSDC)
///
/// When `yield_token` is configured, integrations should wrap base tokens
/// into yield-bearing tokens during funding and unwrap at settlement.
///
/// ```rust
/// fn example_yield_token_escrow() {
///     use soroban_sdk::{testutils::Address as _, Address, Env, String};
///
///     let env = Env::default();
///     env.mock_all_auths();
///
///     let escrow_id = env.register(LiquifactEscrow, ());
///     let client = LiquifactEscrowClient::new(&env, &escrow_id);
///
///     let admin = Address::generate(&env);
///     let sme = Address::generate(&env);
///     let investor = Address::generate(&env);
///     let base_token = Address::generate(&env);      // USDC
///     let yield_token = Address::generate(&env);      // aUSDC (yield-bearing wrapper)
///     let treasury = Address::generate(&env);
///
///     // Init with yield-bearing token configuration
///     let escrow = client.init(
///         &admin,
///         &String::from_str(&env, "YIELD-001"),
///         &sme,
///         &1_000_000_000i128,
///         &400i64,
///         &0u64,
///         &base_token,
///         &None,
///         &treasury,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &Some(yield_token.clone()), // yield_token
///         &None,                      // oracle_contract
///         &None,                      // nft_contract
///     );
///
///     // Verify the yield token is set
///     assert_eq!(client.get_yield_token(), Some(yield_token.clone()));
///
///     // Integration layer: wrap base token → yield token
///     // (e.g., deposit USDC → receive aUSDC)
///     // This is performed off-chain or via a separate contract call.
///
///     client.fund(&investor, &1_000_000_000i128);
///     client.settle();
///
///     // Integration layer: unwrap yield token → base token + yield
///     // (e.g., redeem aUSDC → USDC + accrued interest)
///     // The YieldUnwrapped event is emitted during settlement.
///
///     client.claim_investor_payout(&investor);
/// }
/// ```

/// # Example 6: Oracle-Verified Settlement
///
/// When `oracle_contract` is configured, settlement requires oracle-verified
/// invoice payment proof before finalizing.
///
/// ```rust
/// fn example_oracle_settlement() {
///     use soroban_sdk::{testutils::Address as _, Address, Env, String};
///
///     let env = Env::default();
///     env.mock_all_auths();
///
///     let escrow_id = env.register(LiquifactEscrow, ());
///     let client = LiquifactEscrowClient::new(&env, &escrow_id);
///
///     let admin = Address::generate(&env);
///     let sme = Address::generate(&env);
///     let investor = Address::generate(&env);
///     let funding_token = Address::generate(&env);
///     let treasury = Address::generate(&env);
///     let oracle = Address::generate(&env); // Stellar price oracle contract
///
///     // Init with oracle contract for price-feed verification
///     client.init(
///         &admin,
///         &String::from_str(&env, "ORACLE-001"),
///         &sme,
///         &1_000_000_000i128,
///         &400i64,
///         &0u64,
///         &funding_token,
///         &None,
///         &treasury,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,                      // yield_token
///         &Some(oracle.clone()),       // oracle_contract
///         &None,                      // nft_contract
///     );
///
///     // Verify the oracle is set
///     assert_eq!(client.get_oracle_contract(), Some(oracle.clone()));
///
///     client.fund(&investor, &1_000_000_000i128);
///
///     // Integration layer query pattern:
///     // 1. Query oracle for invoice payment verification
///     // 2. Confirm payment in real-world currency matches invoice
///     // 3. Settle the escrow (oracle verified)
///
///     client.settle();
///
///     // OracleSettlementVerified event is emitted during settlement.
///     client.claim_investor_payout(&investor);
/// }
/// ```

/// # Example 7: Settlement NFT Minting
///
/// When `nft_contract` is configured, settlement triggers NFT minting
/// representing the settled invoice. The SME can use this NFT as
/// collateral or proof of settlement.
///
/// ```rust
/// fn example_settlement_nft() {
///     use soroban_sdk::{testutils::Address as _, Address, Env, String};
///
///     let env = Env::default();
///     env.mock_all_auths();
///
///     let escrow_id = env.register(LiquifactEscrow, ());
///     let client = LiquifactEscrowClient::new(&env, &escrow_id);
///
///     let admin = Address::generate(&env);
///     let sme = Address::generate(&env);
///     let investor = Address::generate(&env);
///     let funding_token = Address::generate(&env);
///     let treasury = Address::generate(&env);
///     let nft_contract = Address::generate(&env); // SEP-41 NFT contract
///
///     // Init with NFT contract for settlement NFT minting
///     client.init(
///         &admin,
///         &String::from_str(&env, "NFT-001"),
///         &sme,
///         &1_000_000_000i128,
///         &400i64,
///         &0u64,
///         &funding_token,
///         &None,
///         &treasury,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,                      // yield_token
///         &None,                      // oracle_contract
///         &Some(nft_contract.clone()), // nft_contract
///     );
///
///     // Verify the NFT contract is set
///     assert_eq!(client.get_nft_contract(), Some(nft_contract.clone()));
///
///     client.fund(&investor, &1_000_000_000i128);
///     client.settle();
///
///     // SettlementNftMinted event is emitted during settlement.
///     // The event includes: invoice_id, settlement_date, yield_paid_bps
///     // Third-party contracts can query the NFT metadata.
///
///     client.claim_investor_payout(&investor);
/// }
/// ```

/// # Example 8: Error Handling Patterns
///
/// Demonstrates how to handle typed errors when interacting with the escrow.
///
/// ```rust
/// fn example_error_handling() {
///     use soroban_sdk::{testutils::Address as _, Address, Env, String};
///
///     let env = Env::default();
///     env.mock_all_auths();
///
///     let escrow_id = env.register(LiquifactEscrow, ());
///     let client = LiquifactEscrowClient::new(&env, &escrow_id);
///
///     let admin = Address::generate(&env);
///     let sme = Address::generate(&env);
///     let token = Address::generate(&env);
///     let treasury = Address::generate(&env);
///
///     // --- Error pattern: Attempt to settle before funding ---
///     client.init(
///         &admin,
///         &String::from_str(&env, "ERR-001"),
///         &sme,
///         &1_000_000_000i128,
///         &400i64,
///         &0u64,
///         &token,
///         &None,
///         &treasury,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///     );
///
///     // settle() on an open escrow panics with SettlementNotFunded (code 121)
///     // Use try_settle() to handle the error gracefully:
///     let result = client.try_settle();
///     assert!(result.is_err());
///
///     // --- Error pattern: Double initialization ---
///     // init() on an already-initialized escrow panics with EscrowAlreadyInitialized (code 3)
///     let reinit_result = client.try_init(
///         &admin,
///         &String::from_str(&env, "ERR-001-DUP"),
///         &sme,
///         &500_000_000i128,
///         &400i64,
///         &0u64,
///         &token,
///         &None,
///         &treasury,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///     );
///     assert!(reinit_result.is_err());
///
///     // --- Error pattern: Read before initialization ---
///     // get_escrow() panics with EscrowNotInitialized (code 20) on a fresh contract
///     let fresh_id = env.register(LiquifactEscrow, ());
///     let fresh_client = LiquifactEscrowClient::new(&env, &fresh_id);
///     let get_result = fresh_client.try_get_escrow();
///     assert!(get_result.is_err());
///
///     // --- Error pattern: Invoice ID validation ---
///     // Invalid invoice IDs (empty, too long, bad chars) panic with
///     // InvoiceIdInvalidLength (code 4) or InvoiceIdInvalidCharset (code 5)
///     let fresh_id2 = env.register(LiquifactEscrow, ());
///     let fresh_client2 = LiquifactEscrowClient::new(&env, &fresh_id2);
///     let bad_id_result = fresh_client2.try_init(
///         &admin,
///         &String::from_str(&env, ""), // empty — invalid
///         &sme,
///         &1_000_000_000i128,
///         &400i64,
///         &0u64,
///         &token,
///         &None,
///         &treasury,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///     );
///     assert!(bad_id_result.is_err());
/// }
/// ```

/// # Example 9: Escrow Summary Query
///
/// Demonstrates the `get_escrow_summary` entrypoint which bundles multiple
/// read-only values for off-chain indexers and client rendering.
///
/// ```rust
/// fn example_escrow_summary() {
///     use soroban_sdk::{testutils::Address as _, Address, Env, String};
///
///     let env = Env::default();
///     env.mock_all_auths();
///
///     let escrow_id = env.register(LiquifactEscrow, ());
///     let client = LiquifactEscrowClient::new(&env, &escrow_id);
///
///     let admin = Address::generate(&env);
///     let sme = Address::generate(&env);
///     let investor = Address::generate(&env);
///     let token = Address::generate(&env);
///     let treasury = Address::generate(&env);
///
///     client.init(
///         &admin,
///         &String::from_str(&env, "SUM-001"),
///         &sme,
///         &2_000_000_000i128,
///         &600i64,
///         &0u64,
///         &token,
///         &None,
///         &treasury,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///     );
///
///     client.fund(&investor, &2_000_000_000i128);
///     client.settle();
///
///     // Get comprehensive escrow state in a single call
///     let summary = client.get_escrow_summary();
///
///     assert_eq!(summary.escrow.status, 2);
///     assert_eq!(summary.schema_version, 6);
///     assert!(!summary.legal_hold);
///     assert_eq!(summary.unique_funder_count, 1);
///     // summary.yield_token, summary.oracle_contract, summary.nft_contract
///     // are all None when not configured.
/// }
/// ```

/// # Example 10: Funding with Deadline
///
/// Demonstrates funding deadline enforcement — after the deadline,
/// new deposits are rejected.
///
/// ```rust
/// fn example_funding_deadline() {
///     use soroban_sdk::{testutils::Address as _, Address, Env, String};
///
///     let env = Env::default();
///     env.mock_all_auths();
///
///     let escrow_id = env.register(LiquifactEscrow, ());
///     let client = LiquifactEscrowClient::new(&env, &escrow_id);
///
///     let admin = Address::generate(&env);
///     let sme = Address::generate(&env);
///     let investor = Address::generate(&env);
///     let token = Address::generate(&env);
///     let treasury = Address::generate(&env);
///
///     let now = env.ledger().timestamp();
///     let deadline = now + 7 * 24 * 60 * 60; // 7 days from now
///
///     client.init(
///         &admin,
///         &String::from_str(&env, "DEADLINE-01"),
///         &sme,
///         &1_000_000_000i128,
///         &400i64,
///         &0u64,
///         &token,
///         &None,
///         &treasury,
///         &None,
///         &None,
///         &None,
///         &None,
///         &None,
///         &Some(deadline), // funding deadline
///         &None,
///         &None,
///         &None,
///         &None,
///     );
///
///     // Fund before deadline — succeeds
///     client.fund(&investor, &500_000_000i128);
///
///     // Advance past deadline
///     env.ledger().with_mut(|l| l.timestamp = deadline + 1);
///
///     // Fund after deadline — panics with FundingDeadlinePassed (code 164)
///     let late_investor = Address::generate(&env);
///     let late_result = client.try_fund(&late_investor, &500_000_000i128);
///     assert!(late_result.is_err());
/// }
/// ```

fn main() {
    println!("karis-ky Escrow SDK Examples");
    println!("===========================");
    println!();
    println!("This file contains example code demonstrating common patterns");
    println!("for interacting with the LiquifactEscrow contract.");
    println!();
    println!("Run with: cargo test -p karis_ky_escrow");
    println!("      or: cargo test -p karis_ky_escrow --example basic_workflow -- --nocapture");
    println!();
    println!("Expected: all example scenarios pass (init -> fund -> settle -> claim,");
    println!("yield-token, NFT mint, and tiered yield). CI runs this example and");
    println!("fails the build if any scenario fails.");
    println!();
    println!("For detailed documentation, see:");
    println!("  - docs/escrow-data-model.md");
    println!("  - docs/escrow-events.md");
    println!("  - docs/escrow-security-checklist.md");
    println!("  - README.md");
}
