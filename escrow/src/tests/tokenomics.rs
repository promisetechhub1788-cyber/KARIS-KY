//! Proptest-based tokenomics modeling tests for yield distribution invariants.
//!
//! These tests verify that yield distribution mechanisms maintain key invariants across
//! various scenarios including inflation (high yield rates), deflation (low/zero yield),
//! and market shifts (changing investor participation).
//!
//! Core invariants verified:
//! 1. Yield always distributes pro-rata (no creation or destruction of value)
//! 2. Sum of individual payouts ≤ total_principal + yield_pool
//! 3. Rounding residuals are bounded and swept by dust sweep
//! 4. Effective yield rates are captured per investor on first deposit

use super::*;
use proptest::prelude::*;
use std::collections::BTreeMap;

/// Strategy for generating realistic funding amounts (base unit scale).
fn gen_funding_amount() -> impl Strategy<Value = i128> {
    1_000i128..=100_000_000i128
}

/// Strategy for generating investor counts.
fn gen_investor_count() -> impl Strategy<Value = usize> {
    1usize..=20usize
}

/// Strategy for generating yield rates (bps).
/// Range: 0 bps (deflation) to 5000 bps (50% yield = extreme inflation).
fn gen_yield_rate_bps() -> impl Strategy<Value = i64> {
    0i64..=5_000i64
}

/// Strategy for generating lock durations (seconds).
fn gen_lock_duration() -> impl Strategy<Value = u64> {
    0u64..=86_400u64 // 0 to 24 hours
}

/// Compute expected settlement pool: principal + coupon (floor).
fn expected_settle_pool(principal: i128, yield_bps: i64) -> i128 {
    let coupon = principal
        .checked_mul(yield_bps as i128)
        .expect("yield coupon overflow")
        .checked_div(10_000)
        .expect("yield coupon divide by zero");
    principal.checked_add(coupon).expect("settle pool overflow")
}

/// Compute expected pro-rata payout for an investor (floor division).
fn expected_payout(investor_contribution: i128, total_principal: i128, settle_pool: i128) -> i128 {
    investor_contribution
        .checked_mul(settle_pool)
        .expect("payout numerator overflow")
        .checked_div(total_principal)
        .expect("payout denominator division by zero")
}

proptest! {
    /// Test: Yield is never created or destroyed across base yield scenarios.
    ///
    /// For a single investor funding alone:
    /// - Their payout should equal principal + (principal × yield_bps / 10_000)
    /// - This verifies the coupon calculation matches their contribution exactly.
    #[test]
    fn prop_single_investor_yield_not_created_or_destroyed(
        funding_amount in gen_funding_amount(),
        yield_bps in gen_yield_rate_bps(),
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let sme = Address::generate(&env);
        let investor = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);

        let client = deploy(&env);

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "SINGLE"),
            &sme,
            &funding_amount,
            &yield_bps,
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
        );

        // Single investor funds to target (no overfunding).
        client.fund(&investor, &funding_amount);

        // Settle after maturity (no ledger time restrictions in this test).
        let settled = client.settle();
        prop_assert_eq!(settled.status, 2, "escrow should be settled");

        // Compute expected payout.
        let expected_pool = expected_settle_pool(funding_amount, yield_bps);
        let expected_inv_payout = expected_payout(funding_amount, funding_amount, expected_pool);

        // Fetch actual payout via contract view.
        let actual_payout = client.compute_investor_payout(&investor);

        prop_assert_eq!(
            actual_payout, expected_inv_payout,
            "single investor payout must match expected pro-rata calculation"
        );
    }

    /// Test: Multiple investors with equal contributions receive equal yields.
    ///
    /// Invariant: pro-rata distribution guarantees each investor with equal contribution
    /// receives exactly equal payout (up to rounding).
    #[test]
    fn prop_equal_contributions_equal_payouts(
        base_contribution in gen_funding_amount(),
        investor_count in gen_investor_count(),
        yield_bps in gen_yield_rate_bps(),
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let sme = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);

        let client = deploy(&env);

        let target = base_contribution
            .checked_mul(investor_count as i128)
            .expect("target overflow");

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "EQUAL"),
            &sme,
            &target,
            &yield_bps,
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
        );

        let mut investors = Vec::new();
        for _ in 0..investor_count {
            let investor = Address::generate(&env);
            investors.push(investor.clone());
            client.fund(&investor, &base_contribution);
        }

        client.settle();

        // Compute expected payout per investor.
        let total_principal = base_contribution
            .checked_mul(investor_count as i128)
            .expect("total principal overflow");
        let settle_pool = expected_settle_pool(total_principal, yield_bps);
        let expected_payout_per_investor = expected_payout(base_contribution, total_principal, settle_pool);

        // Verify all investors have the same payout.
        for investor in investors.iter() {
            let actual_payout = client.compute_investor_payout(investor);
            prop_assert_eq!(
                actual_payout, expected_payout_per_investor,
                "all equal contributors must receive equal payouts"
            );
        }
    }

    /// Test: Sum of all payouts ≤ total_principal + coupon (pro-rata invariant).
    ///
    /// The rounding residual (if any) is captured at the dust sweep boundary
    /// and must be ≤ investor_count (one base unit per investor max).
    #[test]
    fn prop_sum_of_payouts_bounded_by_settle_pool(
        funding_per_investor in gen_funding_amount(),
        investor_count in gen_investor_count(),
        yield_bps in gen_yield_rate_bps(),
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let sme = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);

        let client = deploy(&env);

        let target = funding_per_investor
            .checked_mul(investor_count as i128)
            .expect("target overflow");

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "SUMPOOL"),
            &sme,
            &target,
            &yield_bps,
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
        );

        let mut investors = Vec::new();
        for _ in 0..investor_count {
            let investor = Address::generate(&env);
            investors.push(investor.clone());
            client.fund(&investor, &funding_per_investor);
        }

        client.settle();

        let total_principal = target;
        let settle_pool = expected_settle_pool(total_principal, yield_bps);

        let mut sum_of_payouts: i128 = 0;
        for investor in investors.iter() {
            let payout = client.compute_investor_payout(investor);
            sum_of_payouts = sum_of_payouts
                .checked_add(payout)
                .expect("sum of payouts overflow");
        }

        prop_assert!(
            sum_of_payouts <= settle_pool,
            "sum of payouts ({}) must not exceed settle pool ({})",
            sum_of_payouts,
            settle_pool
        );

        // Rounding residual should be bounded (and available for dust sweep).
        let residual = settle_pool.saturating_sub(sum_of_payouts);
        prop_assert!(
            residual < investor_count as i128,
            "rounding residual ({}) should be less than investor count ({})",
            residual,
            investor_count
        );
    }

    /// Test: Tiered yield correctly increases effective yield for committed investors.
    ///
    /// Invariant: an investor using fund_with_commitment with sufficient lock
    /// should receive a higher yield tier than base yield (if tier table configured).
    #[test]
    fn prop_tiered_yield_increases_investor_return(
        base_contribution in gen_funding_amount(),
        lock_secs in gen_lock_duration(),
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let sme = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);

        let client = deploy(&env);

        let base_yield_bps: i64 = 800; // 8% base yield
        let target = base_contribution;

        // Set up tiered yield table: 100 sec lock → 900 bps, 200 sec lock → 1000 bps.
        let mut tiers = SorobanVec::new(&env);
        tiers.push_back(YieldTier {
            min_lock_secs: 100u64,
            yield_bps: 900i64,
        });
        tiers.push_back(YieldTier {
            min_lock_secs: 200u64,
            yield_bps: 1_000i64,
        });

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "TIER"),
            &sme,
            &target,
            &base_yield_bps,
            &0u64,
            &token,
            &Some(tiers),
            &treasury,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        let investor = Address::generate(&env);

        // Case 1: fund_with_commitment with lock >= 100 sec.
        if lock_secs >= 100 {
            client.fund_with_commitment(&investor, &base_contribution, &lock_secs);
            let effective_yield = client.get_investor_yield_bps(&investor);

            if lock_secs >= 200 {
                prop_assert_eq!(
                    effective_yield, 1_000i64,
                    "investor with 200+ sec lock should get 1000 bps yield"
                );
            } else {
                prop_assert_eq!(
                    effective_yield, 900i64,
                    "investor with 100-199 sec lock should get 900 bps yield"
                );
            }
        } else {
            // Lock < 100 sec → falls through to base yield.
            client.fund_with_commitment(&investor, &base_contribution, &lock_secs);
            let effective_yield = client.get_investor_yield_bps(&investor);
            prop_assert_eq!(
                effective_yield, base_yield_bps,
                "investor with insufficient lock should get base yield"
            );
        }
    }

    /// Test: Zero yield (deflation scenario) produces payouts equal to principal.
    ///
    /// When yield_bps = 0, coupon = 0, settle_pool = principal,
    /// and each investor's payout = their contribution.
    #[test]
    fn prop_zero_yield_equals_principal(
        funding_per_investor in gen_funding_amount(),
        investor_count in gen_investor_count(),
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let sme = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);

        let client = deploy(&env);

        let target = funding_per_investor
            .checked_mul(investor_count as i128)
            .expect("target overflow");

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "ZERO"),
            &sme,
            &target,
            &0i64, // Zero yield (deflation).
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
        );

        let mut investors = Vec::new();
        for _ in 0..investor_count {
            let investor = Address::generate(&env);
            investors.push(investor.clone());
            client.fund(&investor, &funding_per_investor);
        }

        client.settle();

        // With zero yield, each investor should receive exactly their contribution.
        for investor in investors.iter() {
            let payout = client.compute_investor_payout(investor);
            prop_assert_eq!(
                payout, funding_per_investor,
                "zero-yield investor payout should equal their contribution"
            );
        }
    }

    /// Test: High yield (inflation scenario) produces proportionally higher payouts.
    ///
    /// Verify that a 50% yield (5000 bps) produces ~1.5x principal in settle pool,
    /// and payouts scale accordingly.
    #[test]
    fn prop_high_yield_inflation_scenario(
        funding_per_investor in gen_funding_amount(),
        investor_count in gen_investor_count(),
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let sme = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);

        let client = deploy(&env);

        let target = funding_per_investor
            .checked_mul(investor_count as i128)
            .expect("target overflow");

        let high_yield_bps: i64 = 5_000; // 50% yield (extreme inflation).

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "HYIELD"),
            &sme,
            &target,
            &high_yield_bps,
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
        );

        let mut investors = Vec::new();
        for _ in 0..investor_count {
            let investor = Address::generate(&env);
            investors.push(investor.clone());
            client.fund(&investor, &funding_per_investor);
        }

        client.settle();

        let total_principal = target;
        let settle_pool = expected_settle_pool(total_principal, high_yield_bps);

        // At 50% yield: settle_pool ≈ 1.5 × principal.
        let expected_min_pool = total_principal
            .checked_mul(3)
            .expect("expected min pool overflow")
            .checked_div(2)
            .expect("expected min pool division");

        prop_assert!(
            settle_pool >= expected_min_pool,
            "50% yield settle pool ({}) should be at least 1.5x principal ({})",
            settle_pool,
            expected_min_pool
        );

        // Each investor's payout should scale proportionally.
        for investor in investors.iter() {
            let payout = client.compute_investor_payout(investor);
            let expected_payout =
                expected_payout(funding_per_investor, total_principal, settle_pool);

            prop_assert_eq!(
                payout, expected_payout,
                "inflation scenario payout should match pro-rata calculation"
            );
        }
    }

    /// Test: Overfunding (funding > target) captures correct snapshot.
    ///
    /// The funding close snapshot should record the actual funded_amount (including overfunding)
    /// as the pro-rata denominator.
    #[test]
    fn prop_overfunding_snapshot_uses_actual_funded_amount(
        base_contribution in gen_funding_amount(),
        overfund_factor in 1.1f64..=2.0f64, // 10% to 100% overfunding
        yield_bps in gen_yield_rate_bps(),
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let sme = Address::generate(&env);
        let investor = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);

        let client = deploy(&env);

        let target = base_contribution;
        let overfunding_amount =
            ((base_contribution as f64) * (overfund_factor - 1.0)).ceil() as i128;
        let total_funded = base_contribution
            .checked_add(overfunding_amount)
            .expect("overfunded amount overflow");

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "OVER"),
            &sme,
            &target,
            &yield_bps,
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
        );

        client.fund(&investor, &total_funded);

        // Verify escrow is funded after single call with overfunding.
        let escrow = client.get_escrow();
        prop_assert_eq!(escrow.status, 1, "escrow should be funded after overfunding");
        prop_assert_eq!(escrow.funded_amount, total_funded, "funded_amount should capture overfunding");

        client.settle();

        // Payout should use total_funded as the pro-rata denominator, not target.
        let settle_pool = expected_settle_pool(total_funded, yield_bps);
        let expected_payout_base = expected_payout(total_funded, total_funded, settle_pool);

        let actual_payout = client.compute_investor_payout(&investor);

        prop_assert_eq!(
            actual_payout, expected_payout_base,
            "overfunding should use actual funded_amount as pro-rata denominator"
        );
    }

    /// Test: Varying investor contributions produce correct pro-rata payouts.
    ///
    /// With investor A contributing 30% and investor B contributing 70%,
    /// their payouts should maintain the same 30/70 split.
    #[test]
    fn prop_varying_contributions_maintain_pro_rata_ratio(
        base_unit in 10_000i128..=1_000_000i128,
        yield_bps in gen_yield_rate_bps(),
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let sme = Address::generate(&env);
        let investor_a = Address::generate(&env);
        let investor_b = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);

        let client = deploy(&env);

        // A contributes 30%, B contributes 70%.
        let contrib_a = base_unit.checked_mul(3).expect("contrib a overflow");
        let contrib_b = base_unit.checked_mul(7).expect("contrib b overflow");
        let target = contrib_a.checked_add(contrib_b).expect("target overflow");

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "PROP"),
            &sme,
            &target,
            &yield_bps,
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
        );

        client.fund(&investor_a, &contrib_a);
        client.fund(&investor_b, &contrib_b);

        client.settle();

        let settle_pool = expected_settle_pool(target, yield_bps);
        let payout_a = expected_payout(contrib_a, target, settle_pool);
        let payout_b = expected_payout(contrib_b, target, settle_pool);

        let actual_payout_a = client.compute_investor_payout(&investor_a);
        let actual_payout_b = client.compute_investor_payout(&investor_b);

        prop_assert_eq!(actual_payout_a, payout_a, "investor A payout mismatch");
        prop_assert_eq!(actual_payout_b, payout_b, "investor B payout mismatch");

        // Verify pro-rata ratio is maintained.
        if settle_pool > 0 {
            let ratio_a_expected = (contrib_a as f64) / (target as f64);
            let ratio_a_actual = (payout_a as f64) / (settle_pool as f64);

            // Allow small tolerance for rounding.
            prop_assert!(
                (ratio_a_actual - ratio_a_expected).abs() < 0.01,
                "pro-rata ratio should be maintained within rounding tolerance"
            );
        }
    }
}

/// Integration test: complete yield lifecycle across funding, settlement, and claims.
#[test]
fn test_yield_lifecycle_complete() {
    use soroban_sdk::testutils::Ledger as _;

    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    let client = deploy(&env);

    let yield_bps: i64 = 1_200; // 12% APY
    let target = 1_000_000i128;

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "YIELD"),
        &sme,
        &target,
        &yield_bps,
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
    );

    // Three investors with contributions: 300k, 400k, 300k.
    let investor1 = Address::generate(&env);
    let investor2 = Address::generate(&env);
    let investor3 = Address::generate(&env);

    client.fund(&investor1, &300_000i128);
    client.fund(&investor2, &400_000i128);
    client.fund(&investor3, &300_000i128);

    // Verify escrow is funded.
    let escrow = client.get_escrow();
    assert_eq!(escrow.status, 1);
    assert_eq!(escrow.funded_amount, 1_000_000i128);

    // Advance ledger time to allow settlement.
    env.ledger().with_mut(|ledger| {
        ledger.timestamp = 1_000_000;
    });

    // Settle escrow.
    client.settle();
    let settled = client.get_escrow();
    assert_eq!(settled.status, 2);

    // Compute expected payouts.
    let settle_pool = expected_settle_pool(1_000_000i128, yield_bps);
    let payout1 = expected_payout(300_000i128, 1_000_000i128, settle_pool);
    let payout2 = expected_payout(400_000i128, 1_000_000i128, settle_pool);
    let payout3 = expected_payout(300_000i128, 1_000_000i128, settle_pool);

    // Verify computed payouts match expected.
    assert_eq!(client.compute_investor_payout(&investor1), payout1);
    assert_eq!(client.compute_investor_payout(&investor2), payout2);
    assert_eq!(client.compute_investor_payout(&investor3), payout3);

    // Verify sum of payouts ≤ settle pool.
    let sum = payout1
        .checked_add(payout2)
        .expect("sum overflow")
        .checked_add(payout3)
        .expect("sum overflow");
    assert!(sum <= settle_pool);

    // Verify rounding residual is bounded.
    let residual = settle_pool - sum;
    assert!(residual < 3); // At most 2 base units residual for 3 investors.
}
