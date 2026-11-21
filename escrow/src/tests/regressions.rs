//! Regression test suite for production incidents.
//!
//! Each test in this module corresponds to a specific production incident
//! and is named after the incident ID (e.g., `test_regression_201_yield_overflow`).
//! Doc comments link each test to its incident report for traceability.
//!
//! All tests in this module are automatically run on every PR via CI (`cargo test`).
//!
//! ## Incident index
//!
//! | Test | Incident | Description |
//! |------|----------|-------------|
//! | `test_regression_201_yield_overflow` | INC-201 | Yield calculation overflowed for large principal amounts |
//! | `test_regression_202_zero_amount_fund` | INC-202 | Zero-amount fund call caused silent state corruption |
//! | `test_regression_203_double_settle` | INC-203 | Double settlement bypassed status check under race condition |
//! | `test_regression_204_claim_before_lock` | INC-204 | Investor claimed before commitment lock expired via edge-case timestamp |
//! | `test_regression_205_allowlist_bypass` | INC-205 | Allowlist bypass when active flag was toggled mid-funding |
//! | `test_regression_206_dust_sweep_underflow` | INC-206 | Dust sweep arithmetic underflow with zero funded_amount |
//! | `test_regression_207_funding_deadline_edge` | INC-207 | Funding accepted at exact deadline boundary |
//! | `test_regression_208_invoice_id_null_byte` | INC-208 | Invoice ID with embedded null byte passed validation |
//! | `test_regression_210_archive_with_unclaimed_payouts` | INC-210 | Escrow archived while investor payout was still unclaimed |
//!
//! ## How to add a new regression test
//!
//! 1. Add a test function named `test_regression_NNN_short_description`.
//! 2. Add a doc comment linking to the incident report (e.g., `docs/incidents/INC-NNN.md`).
//! 3. Add a row to the incident index table above.
//! 4. The test must fail (panic/should_panic) when the bug is present and pass when fixed.

#[cfg(test)]
use super::{
    assert_contract_error, default_init, deploy, deploy_with_id, free_addresses,
    install_stellar_asset_token, setup, TARGET,
};
use crate::{
    DataKey, EscrowError, InvoiceEscrow, LiquifactEscrow, LiquifactEscrowClient, YieldTier,
};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    token::StellarAssetClient,
    Address, Env, String, Vec,
};

// ──────────────────────────────────────────────────────────────────────────────
// INC-201: Yield calculation overflowed for large principal amounts
//          See: docs/incidents/INC-201.md
// ──────────────────────────────────────────────────────────────────────────────

/// Regression test for INC-201: yield calculation overflow.
///
/// In production, when `total_principal * yield_bps` exceeded `i128::MAX`,
/// the `compute_investor_payout` function would overflow and panic instead
/// of returning a controlled error.
///
/// **Fix:** Changed to `checked_mul` with `ComputePayoutArithmeticOverflow` error.
#[test]
fn test_regression_201_yield_overflow() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);

    // Use near-max amounts to trigger potential overflow
    let huge_amount = i128::MAX / 2;
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "REG201"),
        &sme,
        &huge_amount,
        &10_000i64, // 100% yield — maximizes coupon
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    client.fund(&investor, &huge_amount);
    client.settle();

    // compute_investor_payout should succeed or fail with a typed error,
    // not panic with an arithmetic overflow.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.compute_investor_payout(&investor);
    }));

    // The computation may fail with ComputePayoutArithmeticOverflow,
    // but must not panic due to unchecked arithmetic.
    // If it returns a value, that's fine too — the key is no untrapped overflow.
    assert!(
        result.is_ok(),
        "INC-201: compute_investor_payout must not panic on large amounts"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// INC-202: Zero-amount fund call caused silent state corruption
//          See: docs/incidents/INC-202.md
// ──────────────────────────────────────────────────────────────────────────────

/// Regression test for INC-202: zero-amount funding.
///
/// A zero-amount `fund()` call was accepted (no amount > 0 guard existed),
/// causing `UniqueFunderCount` to increment without actual principal.
#[test]
#[should_panic]
fn test_regression_202_zero_amount_fund() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    let investor = Address::generate(&env);
    // Zero amount must be rejected with FundingAmountNotPositive
    client.fund(&investor, &0i128);
}

// ──────────────────────────────────────────────────────────────────────────────
// INC-203: Double settlement bypassed status check
//          See: docs/incidents/INC-203.md
// ──────────────────────────────────────────────────────────────────────────────

/// Regression test for INC-203: double settlement.
///
/// A race condition allowed `settle()` to be called twice, emitting
/// duplicate `EscrowSettled` events and confusing off-chain indexers.
#[test]
#[should_panic]
fn test_regression_203_double_settle() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    let investor = Address::generate(&env);
    client.fund(&investor, &TARGET);
    client.settle();
    // Second settle must panic — status is already 2
    client.settle();
}

// ──────────────────────────────────────────────────────────────────────────────
// INC-204: Investor claimed before commitment lock expired
//          See: docs/incidents/INC-204.md
// ──────────────────────────────────────────────────────────────────────────────

/// Regression test for INC-204: claim before lock expiry.
///
/// An edge case where the claim-not-before timestamp was off-by-one,
/// allowing a claim 1 second before the lock expired.
#[test]
#[should_panic]
fn test_regression_204_claim_before_lock() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, treasury) = free_addresses(&env);

    env.ledger().set_timestamp(1000);

    client.init(
        &admin,
        &String::from_str(&env, "REG204"),
        &sme,
        &1_000i128,
        &400i64,
        &0u64,
        &tok,
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
        &None,
    );

    let lock_secs = 500u64;
    client.fund_with_commitment(&inv, &1_000i128, &lock_secs);
    client.settle();

    // Set timestamp to 1 second BEFORE expiry
    let expiry = 1000u64 + lock_secs; // 1500
    env.ledger().set_timestamp(expiry - 1); // 1499

    // Claim must be blocked
    client.claim_investor_payout(&inv);
}

// ──────────────────────────────────────────────────────────────────────────────
// INC-205: Allowlist bypass when active flag was toggled mid-funding
//          See: docs/incidents/INC-205.md
// ──────────────────────────────────────────────────────────────────────────────

/// Regression test for INC-205: allowlist bypass.
///
/// A non-allowlisted investor could fund when the allowlist was active
/// due to a stale read of the allowlist flag.
#[test]
#[should_panic]
fn test_regression_205_allowlist_bypass() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "REG205"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &token,
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
        &None,
    );

    // Enable allowlist but do NOT add the investor
    client.set_allowlist_active(&true);
    let investor = Address::generate(&env);

    // Non-allowlisted investor must NOT be able to fund
    client.fund(&investor, &1_000i128);
}

// ──────────────────────────────────────────────────────────────────────────────
// INC-206: Dust sweep arithmetic underflow with zero funded_amount
//          See: docs/incidents/INC-206.md
// ──────────────────────────────────────────────────────────────────────────────

/// Regression test for INC-206: dust sweep with zero funded_amount.
///
/// When an escrow was cancelled before any funding, `sweep_terminal_dust`
/// computed `outstanding = funded_amount - distributed_principal` as 0,
/// but a stray balance check allowed sweeping tokens that should have been
/// reserved for future refunds.
#[test]
fn test_regression_206_dust_sweep_zero_funded() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let token = install_stellar_asset_token(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "REG206"),
        &sme,
        &1_000i128,
        &0i64,
        &0u64,
        &token.id,
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
        &None,
    );

    // Cancel before any funding
    client.cancel_funding();

    // Mint some stray tokens
    token.stellar.mint(&client.address, &50i128);

    // With zero funded_amount, liability floor should allow sweeping dust
    let swept = client.sweep_terminal_dust(&50i128);
    assert_eq!(swept, 50i128);
    assert_eq!(token.token.balance(&treasury), 50i128);
}

// ──────────────────────────────────────────────────────────────────────────────
// INC-207: Funding accepted at exact deadline boundary
//          See: docs/incidents/INC-207.md
// ──────────────────────────────────────────────────────────────────────────────

/// Regression test for INC-207: funding deadline boundary.
///
/// The funding deadline check used `timestamp < deadline` instead of
/// `timestamp <= deadline`, rejecting funding at the exact deadline
/// second when it should have been accepted.
#[test]
fn test_regression_207_funding_deadline_edge() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);
    let deadline = 2000u64;

    env.ledger().set_timestamp(1000);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "REG207"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &Some(deadline),
        &None,
        &None,
        &None,
    );

    // Set timestamp to exact deadline
    env.ledger().set_timestamp(deadline);
    let investor = Address::generate(&env);

    // Funding at exact deadline must succeed
    let escrow = client.fund(&investor, &1_000i128);
    assert_eq!(escrow.funded_amount, 1_000i128);
}

// ──────────────────────────────────────────────────────────────────────────────
// INC-208: Invoice ID with embedded null byte passed validation
//          See: docs/incidents/INC-208.md
// ──────────────────────────────────────────────────────────────────────────────

/// Regression test for INC-208: null byte in invoice ID.
///
/// An invoice ID with an embedded null byte passed charset validation
/// because the validator used C-style string handling that truncated
/// at the null byte, while storage preserved the full byte sequence.
#[test]
#[should_panic]
fn test_regression_208_invoice_id_null_byte() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (t, tr) = free_addresses(&env);

    // Create a string with an embedded null byte in the middle
    let mut bytes = [b'A'; 10];
    bytes[5] = 0;
    let s = soroban_sdk::String::from_bytes(&env, &bytes[..]);

    // This must panic — null bytes should be rejected
    client.init(
        &admin, &s, &sme, &1000i128, &500i64, &0u64, &t, &None, &tr, &None, &None, &None, &None,
        &None, &None,
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Additional regression: overflow in funding target update
// ──────────────────────────────────────────────────────────────────────────────

/// Regression test: funding target update with large values.
///
/// Ensures `update_funding_target` doesn't allow setting targets
/// below already-funded amounts, which would corrupt state.
#[test]
#[should_panic]
fn test_regression_209_target_below_funded() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "REG209"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &token,
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
        &None,
    );

    let investor = Address::generate(&env);
    client.fund(&investor, &50_000_000_000i128);

    // Setting target below already-funded amount must panic
    client.update_funding_target(&40_000_000_000i128);
}

// ──────────────────────────────────────────────────────────────────────────────
// INC-210: Escrow archived while investor payout was still unclaimed
//          See: docs/incidents/INC-210.md
// ──────────────────────────────────────────────────────────────────────────────

/// Regression test for INC-210: archiving with unclaimed payouts.
///
/// A settled escrow could be archived even though an investor had not yet
/// claimed their payout, permanently locking the funds. Archiving must be
/// rejected while any investor payout remains unclaimed, and must succeed
/// once every investor has claimed.
#[test]
fn test_regression_210_archive_with_unclaimed_payouts() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "REG210"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &token,
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
        &None,
    );

    let investor = Address::generate(&env);
    client.fund(&investor, &TARGET);
    client.settle();

    // The investor has not claimed yet — archiving must be rejected.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.archive_escrow();
    }));
    assert!(
        result.is_err(),
        "INC-210: archive_escrow must be rejected while a payout is unclaimed"
    );

    // Once the investor claims, archiving must succeed.
    client.claim_investor_payout(&investor);
    client.archive_escrow();
}
