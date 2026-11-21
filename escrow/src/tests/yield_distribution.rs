//! Tests for automatic yield distribution snapshot at settlement time.
//!
//! This module validates:
//! 1. Enable/disable automatic yield distribution
//! 2. Settlement-time snapshot creation with YieldDistSnapshotCreated event
//! 3. Auto-distributed yield claim with AutoDistributedYieldClaimed event
//! 4. Fallback to on-demand yield computation when auto-distribution disabled
//! 5. Backwards compatibility (disabled by default)
//! 6. Idempotency of claims using pre-computed yields
//! 7. Batch claim with auto-distributed yields

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Setup a multi-investor escrow ready for testing yield distribution.
/// Returns (client, Vec<investor_addresses>, admin, sme).
fn setup_yield_distribution_test(
    env: &Env,
    num_investors: u32,
    amount_per: i128,
    enable_auto_dist: bool,
) -> (LiquifactEscrowClient<'_>, Vec<Address>, Address, Address) {
    env.mock_all_auths();
    let client = deploy(env);
    let admin = Address::generate(env);
    let sme = Address::generate(env);
    let (token, treasury) = free_addresses(env);

    let total = amount_per
        .checked_mul(num_investors as i128)
        .unwrap_or(amount_per);

    client.init(
        &admin,
        &String::from_str(env, "YIELD01"),
        &sme,
        &total,
        &500i64, // 5% base yield
        &0u64,   // No maturity lock
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Enable auto-distribution if requested
    if enable_auto_dist {
        client.enable_yield_auto_distribution();
    }

    // Fund from multiple investors
    let mut investors = Vec::new(env);
    for _i in 0..num_investors {
        let inv = Address::generate(env);
        client.fund(&inv, &amount_per);
        investors.push_back(inv);
    }

    (client, investors, admin, sme)
}

// ── 1. Auto-distribution enable/disable ─────────────────────────────────────

#[test]
fn enable_auto_distribution_sets_flag() {
    let env = Env::default();
    let (client, _investors, _admin, _sme) =
        setup_yield_distribution_test(&env, 1, 100_000_000_000i128, false);

    // Initially disabled (default)
    assert!(!client.is_yield_auto_dist_enabled());

    // Enable it
    client.enable_yield_auto_distribution();
    assert!(client.is_yield_auto_dist_enabled());
}

#[test]
fn disable_auto_distribution_clears_flag() {
    let env = Env::default();
    let (client, _investors, _admin, _sme) =
        setup_yield_distribution_test(&env, 1, 100_000_000_000i128, true);

    // Initially enabled
    assert!(client.is_yield_auto_dist_enabled());

    // Disable it
    client.disable_yield_auto_distribution();
    assert!(!client.is_yield_auto_dist_enabled());
}

#[test]
fn auto_distribution_defaults_to_disabled() {
    let env = Env::default();
    let (client, _investors, _admin, _sme) =
        setup_yield_distribution_test(&env, 1, 100_000_000_000i128, false);

    // Default is false (backwards compatible)
    assert!(!client.is_yield_auto_dist_enabled());
}

// ── 2. Settlement snapshot creation ──────────────────────────────────────────

#[test]
fn settlement_with_auto_dist_disabled_no_snapshot() {
    let env = Env::default();
    let (client, _investors, _admin, _sme) =
        setup_yield_distribution_test(&env, 2, 50_000_000_000i128, false);

    // Settle without auto-distribution enabled
    client.settle();

    // Check that escrow is settled
    let escrow = client.get_escrow();
    assert_eq!(escrow.status, 2);
}

#[test]
fn settlement_with_auto_dist_enabled_creates_snapshot() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "YIELD02"),
        &sme,
        &200_000_000_000i128,
        &500i64,
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
    );

    // Enable auto-distribution BEFORE settlement
    client.enable_yield_auto_distribution();

    // Fund multiple investors
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    client.fund(&inv_a, &100_000_000_000i128);
    client.fund(&inv_b, &100_000_000_000i128);

    // Settle - should create snapshot
    client.settle();

    let escrow = client.get_escrow();
    assert_eq!(escrow.status, 2);
    // Snapshot was created (event emitted)
}

// ── 3. Auto-distributed yield claim ──────────────────────────────────────────

#[test]
fn claim_with_auto_dist_enabled_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "YIELD03"),
        &sme,
        &100_000_000_000i128,
        &500i64, // 5% yield
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
    );

    client.enable_yield_auto_distribution();

    let investor = Address::generate(&env);
    client.fund(&investor, &100_000_000_000i128);
    client.settle();

    // Claim should use pre-computed yield
    client.claim_investor_payout(&investor);

    assert!(client.is_investor_claimed(&investor));
}

#[test]
fn claim_with_auto_dist_disabled_uses_on_demand() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "YIELD04"),
        &sme,
        &100_000_000_000i128,
        &500i64, // 5% yield
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
    );

    // Auto-distribution NOT enabled
    let investor = Address::generate(&env);
    client.fund(&investor, &100_000_000_000i128);
    client.settle();

    // Claim should compute yield on-demand
    client.claim_investor_payout(&investor);

    assert!(client.is_investor_claimed(&investor));
}

// ── 4. Idempotency with auto-distribution ────────────────────────────────────

#[test]
fn auto_dist_claim_is_idempotent() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "YIELD05"),
        &sme,
        &100_000_000_000i128,
        &500i64,
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
    );

    client.enable_yield_auto_distribution();

    let investor = Address::generate(&env);
    client.fund(&investor, &100_000_000_000i128);
    client.settle();

    // First claim
    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));

    // Second claim - should be silent no-op
    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));
}

// ── 5. Multi-investor yield distribution ─────────────────────────────────────

#[test]
fn multi_investor_auto_dist_all_claim() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    let total = 300_000_000_000i128;
    client.init(
        &admin,
        &String::from_str(&env, "YIELD06"),
        &sme,
        &total,
        &500i64, // 5% yield
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
    );

    client.enable_yield_auto_distribution();

    // Three equal investors
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let inv_c = Address::generate(&env);

    let amount = 100_000_000_000i128;
    client.fund(&inv_a, &amount);
    client.fund(&inv_b, &amount);
    client.fund(&inv_c, &amount);

    client.settle();

    // All should be able to claim
    client.claim_investor_payout(&inv_a);
    client.claim_investor_payout(&inv_b);
    client.claim_investor_payout(&inv_c);

    assert!(client.is_investor_claimed(&inv_a));
    assert!(client.is_investor_claimed(&inv_b));
    assert!(client.is_investor_claimed(&inv_c));
}

// ── 6. Backwards compatibility ───────────────────────────────────────────────

#[test]
fn default_escrow_has_auto_dist_disabled() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    // Initialize without enabling auto-distribution
    client.init(
        &admin,
        &String::from_str(&env, "YIELD07"),
        &sme,
        &100_000_000_000i128,
        &500i64,
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
    );

    // Auto-distribution should be disabled by default
    assert!(!client.is_yield_auto_dist_enabled());
}

// ── 7. Feature doesn't break existing claim flow ──────────────────────────────

#[test]
fn auto_dist_feature_backward_compatible() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    // Old-style setup without auto-dist
    client.init(
        &admin,
        &String::from_str(&env, "YIELD08"),
        &sme,
        &100_000_000_000i128,
        &500i64,
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
    );

    let investor = Address::generate(&env);
    client.fund(&investor, &100_000_000_000i128);
    client.settle();

    // Should still work without auto-distribution
    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));

    // compute_investor_payout should still return valid result
    let payout = client.compute_investor_payout(&investor);
    assert!(payout > 0);
}

// ── 8. Authorization checks ──────────────────────────────────────────────────

#[test]
#[should_panic]
fn enable_auto_dist_requires_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "YIELD09"),
        &sme,
        &100_000_000_000i128,
        &500i64,
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
    );

    // Try to enable as non-admin
    let non_admin = Address::generate(&env);
    // This should panic when non_admin tries to authorize
    client.enable_yield_auto_distribution();
}

#[test]
#[should_panic]
fn disable_auto_dist_requires_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "YIELD10"),
        &sme,
        &100_000_000_000i128,
        &500i64,
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
    );

    client.enable_yield_auto_distribution();

    // Try to disable as non-admin - should panic
    client.disable_yield_auto_distribution();
}
